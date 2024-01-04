mod call;
mod four_byte;
mod noop;
mod pre_state;

pub use self::{
    call::{CallConfig, CallFrame, CallLogFrame},
    four_byte::FourByteFrame,
    noop::NoopFrame,
    pre_state::{PreStateConfig, PreStateFrame},
};
use crate::{
    types::{Bytes, H256, U256},
    utils::from_int_or_hex,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

// https://github.com/ethereum/go-ethereum/blob/a9ef135e2dd53682d106c6a2aede9187026cc1de/eth/tracers/logger/logger.go#L406-L411
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DefaultFrame {
    pub failed: bool,
    #[serde(deserialize_with = "from_int_or_hex")]
    pub energy: U256,
    #[serde(rename = "returnValue")]
    pub return_value: Bytes,
    #[serde(rename = "structLogs")]
    pub struct_logs: Vec<StructLog>,
}

// https://github.com/ethereum/go-ethereum/blob/366d2169fbc0e0f803b68c042b77b6b480836dbc/eth/tracers/logger/logger.go#L413-L426
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StructLog {
    pub depth: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub energy: u64,
    #[serde(rename = "energyCost")]
    pub energy_cost: u64,
    /// ref <https://github.com/ethereum/go-ethereum/blob/366d2169fbc0e0f803b68c042b77b6b480836dbc/eth/tracers/logger/logger.go#L450-L452>
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory: Option<Vec<String>>,
    pub op: String,
    pub pc: u64,
    #[serde(default, rename = "refund", skip_serializing_if = "Option::is_none")]
    pub refund_counter: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stack: Option<Vec<U256>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage: Option<BTreeMap<H256, H256>>,
}

#[derive(Debug, PartialEq, Eq, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum GoCoreTraceFrame {
    Default(DefaultFrame),
    NoopTracer(NoopFrame),
    FourByteTracer(FourByteFrame),
    CallTracer(CallFrame),
    PreStateTracer(PreStateFrame),
}

impl From<DefaultFrame> for GoCoreTraceFrame {
    fn from(value: DefaultFrame) -> Self {
        GoCoreTraceFrame::Default(value)
    }
}

impl From<FourByteFrame> for GoCoreTraceFrame {
    fn from(value: FourByteFrame) -> Self {
        GoCoreTraceFrame::FourByteTracer(value)
    }
}

impl From<CallFrame> for GoCoreTraceFrame {
    fn from(value: CallFrame) -> Self {
        GoCoreTraceFrame::CallTracer(value)
    }
}

impl From<PreStateFrame> for GoCoreTraceFrame {
    fn from(value: PreStateFrame) -> Self {
        GoCoreTraceFrame::PreStateTracer(value)
    }
}

impl From<NoopFrame> for GoCoreTraceFrame {
    fn from(value: NoopFrame) -> Self {
        GoCoreTraceFrame::NoopTracer(value)
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum GoCoreTrace {
    Known(GoCoreTraceFrame),
    Unknown(Value),
}

impl From<GoCoreTraceFrame> for GoCoreTrace {
    fn from(value: GoCoreTraceFrame) -> Self {
        GoCoreTrace::Known(value)
    }
}

impl From<Value> for GoCoreTrace {
    fn from(value: Value) -> Self {
        GoCoreTrace::Unknown(value)
    }
}

/// Available built-in tracers
///
/// See <https://geth.ethereum.org/docs/developers/evm-tracing/built-in-tracers>
#[derive(Debug, PartialEq, Eq, Clone, Deserialize, Serialize)]
pub enum GoCoreDebugBuiltInTracerType {
    #[serde(rename = "4byteTracer")]
    FourByteTracer,
    #[serde(rename = "callTracer")]
    CallTracer,
    #[serde(rename = "prestateTracer")]
    PreStateTracer,
    #[serde(rename = "noopTracer")]
    NoopTracer,
}

#[derive(Debug, PartialEq, Eq, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum GoCoreDebugBuiltInTracerConfig {
    CallTracer(CallConfig),
    PreStateTracer(PreStateConfig),
}

/// Available tracers
///
/// See <https://geth.ethereum.org/docs/developers/evm-tracing/built-in-tracers> and <https://geth.ethereum.org/docs/developers/evm-tracing/custom-tracer>
#[derive(Debug, PartialEq, Eq, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum GoCoreDebugTracerType {
    /// built-in tracer
    BuiltInTracer(GoCoreDebugBuiltInTracerType),

    /// custom JS tracer
    JsTracer(String),
}

#[derive(Debug, PartialEq, Eq, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum GoCoreDebugTracerConfig {
    /// built-in tracer
    BuiltInTracer(GoCoreDebugBuiltInTracerConfig),

    /// custom JS tracer
    JsTracer(Value),
}

/// Bindings for additional `debug_traceTransaction` options
///
/// See <https://geth.ethereum.org/docs/rpc/ns-debug#debug_tracetransaction>
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GoCoreDebugTracingOptions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disable_storage: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disable_stack: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enable_memory: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enable_return_data: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tracer: Option<GoCoreDebugTracerType>,
    /// tracerConfig is slated for GoCore v1.11.0
    /// See <https://github.com/ethereum/go-ethereum/issues/26513>
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tracer_config: Option<GoCoreDebugTracerConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout: Option<String>,
}

/// Bindings for additional `debug_traceCall` options
///
/// See <https://geth.ethereum.org/docs/rpc/ns-debug#debug_tracecall>
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GoCoreDebugTracingCallOptions {
    #[serde(flatten)]
    pub tracing_options: GoCoreDebugTracingOptions,
    // TODO: Add stateoverrides and blockoverrides options
}
