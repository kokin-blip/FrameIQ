use frameiq_contracts::*;
use prost::Message;
#[derive(Clone, PartialEq, Message)]
struct Model {
    #[prost(message, optional, tag = "7")]
    graph: Option<Graph>,
    #[prost(bytes = "vec", repeated, tag = "25")]
    functions: Vec<Vec<u8>>,
}
#[derive(Clone, PartialEq, Message)]
struct Graph {
    #[prost(message, repeated, tag = "1")]
    nodes: Vec<Node>,
    #[prost(message, repeated, tag = "5")]
    tensors: Vec<Tensor>,
    #[prost(bytes = "vec", repeated, tag = "15")]
    sparse: Vec<Vec<u8>>,
}
#[derive(Clone, PartialEq, Message)]
struct Node {
    #[prost(string, tag = "4")]
    op: String,
    #[prost(message, repeated, tag = "5")]
    attributes: Vec<Attribute>,
    #[prost(string, tag = "7")]
    domain: String,
}
#[derive(Clone, PartialEq, Message)]
struct Attribute {
    #[prost(message, optional, tag = "5")]
    tensor: Option<Tensor>,
    #[prost(bytes = "vec", optional, tag = "6")]
    graph: Option<Vec<u8>>,
    #[prost(message, repeated, tag = "10")]
    tensors: Vec<Tensor>,
    #[prost(bytes = "vec", repeated, tag = "11")]
    graphs: Vec<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "22")]
    sparse_tensor: Option<Vec<u8>>,
    #[prost(bytes = "vec", repeated, tag = "23")]
    sparse_tensors: Vec<Vec<u8>>,
}
#[derive(Clone, PartialEq, Message)]
struct Tensor {
    #[prost(int64, repeated, tag = "1")]
    dims: Vec<i64>,
    #[prost(bytes = "vec", repeated, tag = "13")]
    external: Vec<Vec<u8>>,
    #[prost(int32, tag = "14")]
    location: i32,
}
fn bad() -> AppError {
    AppError::new(
        "MODEL_UNSAFE",
        "Use a self-contained ONNX graph with supported standard operators and bounded tensors.",
    )
}
fn tensor(t: &Tensor) -> AppResult<()> {
    if t.location != 0
        || !t.external.is_empty()
        || t.dims.iter().any(|n| *n < 0)
        || t.dims
            .iter()
            .try_fold(1u64, |a, n| a.checked_mul(*n as u64))
            .is_none_or(|n| n > 1_000_000_000)
    {
        Err(bad())
    } else {
        Ok(())
    }
}
pub fn validate(bytes: &[u8]) -> AppResult<()> {
    let model = Model::decode(bytes).map_err(|_| bad())?;
    if !model.functions.is_empty() {
        return Err(bad());
    }
    let graph = model.graph.ok_or_else(bad)?;
    if graph.nodes.len() > 10000 || !graph.sparse.is_empty() {
        return Err(bad());
    }
    let allowed = [
        "Abs",
        "Add",
        "And",
        "ArgMax",
        "AveragePool",
        "BatchNormalization",
        "Cast",
        "Ceil",
        "Clip",
        "Concat",
        "Constant",
        "ConstantOfShape",
        "Conv",
        "ConvTranspose",
        "Cos",
        "DepthToSpace",
        "Div",
        "Dropout",
        "Equal",
        "Erf",
        "Exp",
        "Expand",
        "Flatten",
        "Floor",
        "Gather",
        "GatherElements",
        "Gemm",
        "GlobalAveragePool",
        "Greater",
        "HardSigmoid",
        "HardSwish",
        "Identity",
        "InstanceNormalization",
        "LayerNormalization",
        "LeakyRelu",
        "Less",
        "Log",
        "MatMul",
        "Max",
        "MaxPool",
        "Min",
        "Mul",
        "Neg",
        "NonMaxSuppression",
        "Not",
        "Pad",
        "Pow",
        "PRelu",
        "Range",
        "Reciprocal",
        "ReduceL2",
        "ReduceMax",
        "ReduceMean",
        "ReduceMin",
        "ReduceSum",
        "Relu",
        "Reshape",
        "Resize",
        "Round",
        "Shape",
        "Sigmoid",
        "Sin",
        "Slice",
        "Softmax",
        "Split",
        "Sqrt",
        "Squeeze",
        "Sub",
        "Tanh",
        "TopK",
        "Transpose",
        "Unsqueeze",
        "Where",
    ];
    for node in graph.nodes {
        if !["", "ai.onnx"].contains(&node.domain.as_str()) || !allowed.contains(&node.op.as_str())
        {
            return Err(bad());
        }
        for a in node.attributes {
            if a.graph.is_some()
                || !a.graphs.is_empty()
                || a.sparse_tensor.is_some()
                || !a.sparse_tensors.is_empty()
            {
                return Err(bad());
            }
            if let Some(t) = a.tensor {
                tensor(&t)?;
            }
            for t in a.tensors {
                tensor(&t)?;
            }
        }
    }
    for t in graph.tensors {
        tensor(&t)?;
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn custom_domains_and_external_weights_are_rejected() {
        let m = Model {
            graph: Some(Graph {
                nodes: vec![Node {
                    op: "Identity".into(),
                    domain: "custom".into(),
                    attributes: vec![],
                }],
                tensors: vec![],
                sparse: vec![],
            }),
            functions: vec![],
        };
        assert!(validate(&m.encode_to_vec()).is_err());
        let t = Tensor {
            dims: vec![1],
            external: vec![vec![1]],
            location: 1,
        };
        assert!(tensor(&t).is_err());
    }
}
