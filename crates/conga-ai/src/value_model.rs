//! Value model with a hand-written forward pass (D52).
//!
//! `ValueModel` is a deterministic feed-forward MLP that reads an
//! encoded observation (see `encoding::encode`) and returns a scalar
//! match-equity estimate in `[0, 1]`. Weights are stored as a JSON file
//! that round-trips through serde so the committed artefact is human
//! inspectable and diffable, matching the m5 equity-table precedent
//! (D44).
//!
//! JSON schema (D57):
//!
//! ```json
//! {
//!   "schema_version": 1,
//!   "encoding_version": 1,
//!   "input_size": 261,
//!   "layers": [
//!     {
//!       "out_size": 128,
//!       "activation": "relu",
//!       "weights": [[...<input_size>...], ... <out_size> rows ...],
//!       "biases":  [...<out_size>...]
//!     },
//!     ...
//!     { "out_size": 1, "activation": "sigmoid", "weights": [[...]], "biases": [0.0] }
//!   ],
//!   "meta": { ... arbitrary training provenance ... }
//! }
//! ```
//!
//! Validation at load time rejects:
//! - `encoding_version` not equal to `encoding::ENCODING_VERSION`,
//! - `input_size` not equal to `encoding::ENCODING_SIZE`,
//! - an empty `layers` list,
//! - any layer where `weights.len() != out_size` or `biases.len() != out_size`,
//! - any layer where `weights[i].len()` does not match the previous layer's
//!   `out_size` (or `input_size` for the first layer),
//! - a final layer with `out_size != 1`.
//!
//! The search integration (slice 3) will hold a `ValueModel` behind an
//! `Arc` inside `SearchPlayer` and call `forward` on each depth-D rollout
//! leaf. The matmul is plain `Vec<f32>` arithmetic; a few dozen lines is
//! enough for a 261 → 128 → 128 → 1 network.

use std::fs;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::encoding::{ENCODING_SIZE, ENCODING_VERSION};

/// Current JSON schema version. Bumped together with a code change.
pub const SCHEMA_VERSION: u32 = 1;

/// Activation applied element-wise after each layer's affine transform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Activation {
    Relu,
    Sigmoid,
    Linear,
}

impl Activation {
    fn apply(self, pre: f32) -> f32 {
        match self {
            Activation::Relu => {
                if pre > 0.0 {
                    pre
                } else {
                    0.0
                }
            }
            Activation::Sigmoid => 1.0 / (1.0 + (-pre).exp()),
            Activation::Linear => pre,
        }
    }
}

/// One fully-connected layer. `weights` is row-major: `weights[j][k]`
/// is the weight from input unit `k` to output unit `j`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Layer {
    pub out_size: usize,
    pub activation: Activation,
    pub weights: Vec<Vec<f32>>,
    pub biases: Vec<f32>,
}

/// A feed-forward value model. Load via [`ValueModel::from_json_str`]
/// or [`ValueModel::load_file`]; both validate the shape against the
/// current `ENCODING_SIZE` and `ENCODING_VERSION`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValueModel {
    pub schema_version: u32,
    pub encoding_version: u32,
    pub input_size: usize,
    pub layers: Vec<Layer>,
    #[serde(default = "default_meta")]
    pub meta: serde_json::Value,
}

fn default_meta() -> serde_json::Value {
    serde_json::Value::Null
}

/// Errors returned by model loading / validation. `Io` and `Parse`
/// wrap stdlib / serde errors so callers can discriminate between a
/// missing file, a malformed file and a shape-mismatched file.
#[derive(Debug)]
pub enum ValueModelError {
    Io(io::Error),
    Parse(serde_json::Error),
    EncodingVersionMismatch {
        expected: u32,
        found: u32,
    },
    InputSizeMismatch {
        expected: usize,
        found: usize,
    },
    EmptyLayers,
    LayerShapeMismatch {
        layer: usize,
        out_size: usize,
        weights_rows: usize,
        bias_len: usize,
    },
    LayerChainMismatch {
        layer: usize,
        expected_input: usize,
        found_input: usize,
    },
    FinalLayerOutSizeNotOne {
        found: usize,
    },
}

impl std::fmt::Display for ValueModelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "io error: {e}"),
            Self::Parse(e) => write!(f, "json parse error: {e}"),
            Self::EncodingVersionMismatch { expected, found } => write!(
                f,
                "encoding version mismatch: expected {expected}, found {found}"
            ),
            Self::InputSizeMismatch { expected, found } => {
                write!(f, "input size mismatch: expected {expected}, found {found}")
            }
            Self::EmptyLayers => write!(f, "model must have at least one layer"),
            Self::LayerShapeMismatch {
                layer,
                out_size,
                weights_rows,
                bias_len,
            } => write!(
                f,
                "layer {layer} shape mismatch: out_size={out_size}, weights_rows={weights_rows}, biases_len={bias_len}"
            ),
            Self::LayerChainMismatch {
                layer,
                expected_input,
                found_input,
            } => write!(
                f,
                "layer {layer} input dim mismatch: expected {expected_input} (prev layer output), found {found_input}"
            ),
            Self::FinalLayerOutSizeNotOne { found } => {
                write!(f, "final layer must have out_size == 1, found {found}")
            }
        }
    }
}

impl std::error::Error for ValueModelError {}

impl From<io::Error> for ValueModelError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<serde_json::Error> for ValueModelError {
    fn from(e: serde_json::Error) -> Self {
        Self::Parse(e)
    }
}

impl ValueModel {
    /// Parse and validate a JSON-encoded model from a string.
    pub fn from_json_str(s: &str) -> Result<Self, ValueModelError> {
        let model: ValueModel = serde_json::from_str(s)?;
        model.validate()?;
        Ok(model)
    }

    /// Load and validate a JSON-encoded model from a file.
    pub fn load_file(path: &Path) -> Result<Self, ValueModelError> {
        let s = fs::read_to_string(path)?;
        Self::from_json_str(&s)
    }

    /// Serialise with `serde_json::to_string_pretty`. The forward pass
    /// is deterministic over f32; whether a Python-exported file
    /// round-trips bit-identically through the Rust reader is tested in
    /// slice 5's reproducibility gate.
    pub fn to_json_string_pretty(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Forward pass. `x` must have length `input_size` (not re-checked
    /// per call; `validate` on load guarantees the shape chain).
    pub fn forward(&self, x: &[f32]) -> f32 {
        debug_assert_eq!(x.len(), self.input_size);

        let mut current: Vec<f32> = x.to_vec();
        let mut next: Vec<f32> = Vec::new();

        for layer in &self.layers {
            next.clear();
            next.reserve(layer.out_size);
            for j in 0..layer.out_size {
                let row = &layer.weights[j];
                let mut acc = layer.biases[j];
                for (k, &xk) in current.iter().enumerate() {
                    acc += row[k] * xk;
                }
                next.push(layer.activation.apply(acc));
            }
            std::mem::swap(&mut current, &mut next);
        }

        current[0]
    }

    fn validate(&self) -> Result<(), ValueModelError> {
        if self.encoding_version != ENCODING_VERSION {
            return Err(ValueModelError::EncodingVersionMismatch {
                expected: ENCODING_VERSION,
                found: self.encoding_version,
            });
        }
        if self.input_size != ENCODING_SIZE {
            return Err(ValueModelError::InputSizeMismatch {
                expected: ENCODING_SIZE,
                found: self.input_size,
            });
        }
        if self.layers.is_empty() {
            return Err(ValueModelError::EmptyLayers);
        }

        let mut prev_out = self.input_size;
        for (i, layer) in self.layers.iter().enumerate() {
            if layer.weights.len() != layer.out_size || layer.biases.len() != layer.out_size {
                return Err(ValueModelError::LayerShapeMismatch {
                    layer: i,
                    out_size: layer.out_size,
                    weights_rows: layer.weights.len(),
                    bias_len: layer.biases.len(),
                });
            }
            for row in &layer.weights {
                if row.len() != prev_out {
                    return Err(ValueModelError::LayerChainMismatch {
                        layer: i,
                        expected_input: prev_out,
                        found_input: row.len(),
                    });
                }
            }
            prev_out = layer.out_size;
        }

        let final_out = self.layers.last().map(|l| l.out_size).unwrap_or_default();
        if final_out != 1 {
            return Err(ValueModelError::FinalLayerOutSizeNotOne { found: final_out });
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal valid model: single `input_size → 1` linear layer with
    /// all weights set to `w` and a zero bias. Useful as a shared
    /// fixture across tests.
    fn single_layer_model(w: f32, activation: Activation) -> ValueModel {
        let row: Vec<f32> = vec![w; ENCODING_SIZE];
        ValueModel {
            schema_version: SCHEMA_VERSION,
            encoding_version: ENCODING_VERSION,
            input_size: ENCODING_SIZE,
            layers: vec![Layer {
                out_size: 1,
                activation,
                weights: vec![row],
                biases: vec![0.0],
            }],
            meta: serde_json::Value::Null,
        }
    }

    #[test]
    fn forward_linear_single_layer_is_sum_times_weight() {
        let model = single_layer_model(0.5, Activation::Linear);
        let mut x = vec![0.0f32; ENCODING_SIZE];
        x[0] = 2.0;
        x[1] = 4.0;
        // 0.5 * 2.0 + 0.5 * 4.0 = 3.0
        assert!((model.forward(&x) - 3.0).abs() < 1e-6);
    }

    #[test]
    fn forward_sigmoid_at_zero_is_half() {
        let model = single_layer_model(0.0, Activation::Sigmoid);
        let x = vec![1.0f32; ENCODING_SIZE];
        // weights are 0, so pre-activation = 0, sigmoid(0) = 0.5
        assert!((model.forward(&x) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn forward_relu_gates_negative_input() {
        // Two-layer: first layer has one row of all -1 weights and 0 bias;
        // ReLU drops a negative pre-activation to zero. Second layer is
        // a trivial linear [1.0] mapping that one hidden unit to output.
        let model = ValueModel {
            schema_version: SCHEMA_VERSION,
            encoding_version: ENCODING_VERSION,
            input_size: ENCODING_SIZE,
            layers: vec![
                Layer {
                    out_size: 1,
                    activation: Activation::Relu,
                    weights: vec![vec![-1.0f32; ENCODING_SIZE]],
                    biases: vec![0.0],
                },
                Layer {
                    out_size: 1,
                    activation: Activation::Linear,
                    weights: vec![vec![1.0]],
                    biases: vec![0.0],
                },
            ],
            meta: serde_json::Value::Null,
        };
        // All-positive input ⇒ pre-activation is negative ⇒ ReLU zeros it ⇒ output = 0.
        let x = vec![1.0f32; ENCODING_SIZE];
        assert_eq!(model.forward(&x), 0.0);
    }

    #[test]
    fn serde_roundtrip_preserves_forward_output() {
        let model = single_layer_model(0.25, Activation::Sigmoid);
        let json = model.to_json_string_pretty().unwrap();
        let parsed = ValueModel::from_json_str(&json).unwrap();
        assert_eq!(parsed, model);

        let mut x = vec![0.1f32; ENCODING_SIZE];
        x[7] = 0.9;
        assert_eq!(parsed.forward(&x), model.forward(&x));
    }

    #[test]
    fn serde_roundtrip_of_three_layer_shape_matches_committed_schema() {
        let model = ValueModel {
            schema_version: SCHEMA_VERSION,
            encoding_version: ENCODING_VERSION,
            input_size: ENCODING_SIZE,
            layers: vec![
                Layer {
                    out_size: 4,
                    activation: Activation::Relu,
                    weights: vec![vec![0.01; ENCODING_SIZE]; 4],
                    biases: vec![0.0; 4],
                },
                Layer {
                    out_size: 3,
                    activation: Activation::Relu,
                    weights: vec![vec![0.5; 4]; 3],
                    biases: vec![0.0; 3],
                },
                Layer {
                    out_size: 1,
                    activation: Activation::Sigmoid,
                    weights: vec![vec![1.0; 3]],
                    biases: vec![0.0],
                },
            ],
            meta: serde_json::json!({"train_mse": 0.01}),
        };
        let json = model.to_json_string_pretty().unwrap();
        let parsed = ValueModel::from_json_str(&json).unwrap();
        assert_eq!(parsed, model);
    }

    fn valid_two_layer_json() -> String {
        serde_json::to_string(&ValueModel {
            schema_version: SCHEMA_VERSION,
            encoding_version: ENCODING_VERSION,
            input_size: ENCODING_SIZE,
            layers: vec![
                Layer {
                    out_size: 2,
                    activation: Activation::Relu,
                    weights: vec![vec![0.0; ENCODING_SIZE]; 2],
                    biases: vec![0.0; 2],
                },
                Layer {
                    out_size: 1,
                    activation: Activation::Sigmoid,
                    weights: vec![vec![0.0; 2]],
                    biases: vec![0.0],
                },
            ],
            meta: serde_json::Value::Null,
        })
        .unwrap()
    }

    #[test]
    fn load_rejects_mismatched_encoding_version() {
        let mut v: serde_json::Value = serde_json::from_str(&valid_two_layer_json()).unwrap();
        v["encoding_version"] = serde_json::json!(999);
        let s = serde_json::to_string(&v).unwrap();
        let err = ValueModel::from_json_str(&s).unwrap_err();
        assert!(
            matches!(
                err,
                ValueModelError::EncodingVersionMismatch { found: 999, .. }
            ),
            "got {err:?}"
        );
    }

    #[test]
    fn load_rejects_mismatched_input_size() {
        let mut v: serde_json::Value = serde_json::from_str(&valid_two_layer_json()).unwrap();
        v["input_size"] = serde_json::json!(ENCODING_SIZE + 1);
        let s = serde_json::to_string(&v).unwrap();
        let err = ValueModel::from_json_str(&s).unwrap_err();
        assert!(
            matches!(err, ValueModelError::InputSizeMismatch { .. }),
            "got {err:?}"
        );
    }

    #[test]
    fn load_rejects_empty_layers() {
        let model = ValueModel {
            schema_version: SCHEMA_VERSION,
            encoding_version: ENCODING_VERSION,
            input_size: ENCODING_SIZE,
            layers: vec![],
            meta: serde_json::Value::Null,
        };
        let s = serde_json::to_string(&model).unwrap();
        let err = ValueModel::from_json_str(&s).unwrap_err();
        assert!(matches!(err, ValueModelError::EmptyLayers), "got {err:?}");
    }

    #[test]
    fn load_rejects_final_layer_out_ne_1() {
        let model = ValueModel {
            schema_version: SCHEMA_VERSION,
            encoding_version: ENCODING_VERSION,
            input_size: ENCODING_SIZE,
            layers: vec![Layer {
                out_size: 2,
                activation: Activation::Linear,
                weights: vec![vec![0.0; ENCODING_SIZE]; 2],
                biases: vec![0.0; 2],
            }],
            meta: serde_json::Value::Null,
        };
        let s = serde_json::to_string(&model).unwrap();
        let err = ValueModel::from_json_str(&s).unwrap_err();
        assert!(
            matches!(err, ValueModelError::FinalLayerOutSizeNotOne { found: 2 }),
            "got {err:?}"
        );
    }

    #[test]
    fn load_rejects_layer_chain_mismatch() {
        // Second layer expects input dim = first layer out_size = 2, but we
        // declare a 3-wide weights row.
        let model = ValueModel {
            schema_version: SCHEMA_VERSION,
            encoding_version: ENCODING_VERSION,
            input_size: ENCODING_SIZE,
            layers: vec![
                Layer {
                    out_size: 2,
                    activation: Activation::Relu,
                    weights: vec![vec![0.0; ENCODING_SIZE]; 2],
                    biases: vec![0.0; 2],
                },
                Layer {
                    out_size: 1,
                    activation: Activation::Linear,
                    weights: vec![vec![0.0; 3]],
                    biases: vec![0.0],
                },
            ],
            meta: serde_json::Value::Null,
        };
        let s = serde_json::to_string(&model).unwrap();
        let err = ValueModel::from_json_str(&s).unwrap_err();
        assert!(
            matches!(
                err,
                ValueModelError::LayerChainMismatch {
                    layer: 1,
                    expected_input: 2,
                    found_input: 3,
                }
            ),
            "got {err:?}"
        );
    }

    #[test]
    fn load_rejects_layer_shape_mismatch_rows_vs_out_size() {
        // weights has 2 rows but out_size says 3.
        let model = ValueModel {
            schema_version: SCHEMA_VERSION,
            encoding_version: ENCODING_VERSION,
            input_size: ENCODING_SIZE,
            layers: vec![
                Layer {
                    out_size: 3,
                    activation: Activation::Relu,
                    weights: vec![vec![0.0; ENCODING_SIZE]; 2],
                    biases: vec![0.0; 3],
                },
                Layer {
                    out_size: 1,
                    activation: Activation::Linear,
                    weights: vec![vec![0.0; 3]],
                    biases: vec![0.0],
                },
            ],
            meta: serde_json::Value::Null,
        };
        let s = serde_json::to_string(&model).unwrap();
        let err = ValueModel::from_json_str(&s).unwrap_err();
        assert!(
            matches!(
                err,
                ValueModelError::LayerShapeMismatch {
                    layer: 0,
                    out_size: 3,
                    weights_rows: 2,
                    bias_len: 3,
                }
            ),
            "got {err:?}"
        );
    }

    #[test]
    fn activation_serialises_lowercase() {
        assert_eq!(
            serde_json::to_string(&Activation::Relu).unwrap(),
            "\"relu\""
        );
        assert_eq!(
            serde_json::to_string(&Activation::Sigmoid).unwrap(),
            "\"sigmoid\""
        );
        assert_eq!(
            serde_json::to_string(&Activation::Linear).unwrap(),
            "\"linear\""
        );
    }

    #[test]
    fn missing_meta_defaults_to_null() {
        let json = format!(
            r#"{{"schema_version":1,"encoding_version":1,"input_size":{},"layers":[{{"out_size":1,"activation":"linear","weights":[[{}]],"biases":[0.0]}}]}}"#,
            ENCODING_SIZE,
            vec!["0.0"; ENCODING_SIZE].join(",")
        );
        let parsed = ValueModel::from_json_str(&json).unwrap();
        assert_eq!(parsed.meta, serde_json::Value::Null);
    }
}
