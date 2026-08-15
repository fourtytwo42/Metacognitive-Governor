//! Fail-closed trusted-native and capability-restricted WASI plugin execution.

use std::{collections::BTreeMap, sync::Arc, time::Duration};

pub use mcg_plugin_protocol::{
    PLUGIN_SCHEMA_VERSION, PluginCapability, PluginClass, PluginError, PluginInvocation,
    PluginManifest, PluginResult, WasiPluginLimits,
};
use serde_json::Value;
use wasmtime::{Config, Engine, Linker, Module, Store, StoreLimits, StoreLimitsBuilder};
use wasmtime_wasi::{
    WasiCtxBuilder,
    p1::{self, WasiP1Ctx},
    p2::pipe::{MemoryInputPipe, MemoryOutputPipe},
};

/// Statically linked host extension. Dynamic libraries are deliberately excluded:
/// the repository forbids unsafe loading and the host owns every native authority.
pub trait TrustedNativePlugin: Send + Sync {
    fn manifest(&self) -> PluginManifest;

    /// Executes one host-routed invocation without ambient lookup or fallback.
    ///
    /// # Errors
    /// Returns a plugin-defined, content-safe execution failure.
    fn invoke(&self, input: &Value) -> Result<Value, PluginError>;
}

#[derive(Default)]
pub struct TrustedPluginRegistry {
    plugins: BTreeMap<String, Arc<dyn TrustedNativePlugin>>,
}

impl TrustedPluginRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers one statically linked plugin after validating its immutable manifest.
    ///
    /// # Errors
    /// Returns an error for invalid manifests, the wrong class, or duplicate identity.
    pub fn register(&mut self, plugin: Arc<dyn TrustedNativePlugin>) -> Result<(), PluginError> {
        let manifest = plugin.manifest();
        manifest.validate()?;
        if manifest.class != PluginClass::TrustedNative {
            return Err(PluginError::Manifest(
                "trusted registry accepts only trusted-native plugins".into(),
            ));
        }
        if self.plugins.contains_key(&manifest.id) {
            return Err(PluginError::Duplicate(manifest.id));
        }
        self.plugins.insert(manifest.id, plugin);
        Ok(())
    }

    #[must_use]
    pub fn manifests(&self) -> Vec<PluginManifest> {
        self.plugins
            .values()
            .map(|plugin| plugin.manifest())
            .collect()
    }

    /// Invokes one exact registered plugin without ambient fallback.
    ///
    /// # Errors
    /// Returns validation, lookup, plugin, or result-serialization failures.
    pub fn invoke(&self, invocation: &PluginInvocation) -> Result<PluginResult, PluginError> {
        invocation.validate()?;
        let plugin = self
            .plugins
            .get(&invocation.plugin_id)
            .ok_or_else(|| PluginError::NotFound(invocation.plugin_id.clone()))?;
        let output = plugin.invoke(&invocation.input)?;
        Ok(PluginResult {
            schema_version: PLUGIN_SCHEMA_VERSION,
            plugin_id: invocation.plugin_id.clone(),
            output,
        })
    }
}

struct WasiStoreState {
    wasi: WasiP1Ctx,
    limits: StoreLimits,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WasiPluginOutput {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

/// Executes `WASIp1` command modules with closed ambient authority.
///
/// The context inherits no host environment, arguments, terminal, directories,
/// or network grants. Only bounded in-memory stdin/stdout/stderr are attached.
pub struct WasiPluginExecutor {
    engine: Engine,
    limits: WasiPluginLimits,
}

impl WasiPluginExecutor {
    /// Creates one deterministic Wasmtime engine with fuel accounting enabled.
    ///
    /// # Errors
    /// Returns invalid limit or engine configuration failures.
    pub fn new(limits: WasiPluginLimits) -> Result<Self, PluginError> {
        limits.validate()?;
        let mut config = Config::new();
        config.consume_fuel(true);
        config.wasm_multi_memory(false);
        let engine = Engine::new(&config).map_err(execution_error)?;
        Ok(Self { engine, limits })
    }

    /// Invokes a command module with one bounded JSON input document on stdin.
    ///
    /// # Errors
    /// Returns capability, compilation, import, resource, execution, or output errors.
    pub async fn invoke_json(
        &self,
        manifest: &PluginManifest,
        module_bytes: &[u8],
        input: &Value,
    ) -> Result<PluginResult, PluginError> {
        manifest.validate()?;
        if manifest.class != PluginClass::SandboxedWasi {
            return Err(PluginError::Manifest(
                "WASI executor accepts only sandboxed-WASI plugins".into(),
            ));
        }
        let input = serde_json::to_vec(input)
            .map_err(|error| PluginError::InvalidResult(error.to_string()))?;
        let output = self.invoke_bytes(manifest, module_bytes, &input).await?;
        if !output.stderr.is_empty() {
            return Err(PluginError::Execution(
                "sandboxed plugin wrote to its diagnostic stream".into(),
            ));
        }
        let value = serde_json::from_slice(&output.stdout)
            .map_err(|error| PluginError::InvalidResult(error.to_string()))?;
        Ok(PluginResult {
            schema_version: PLUGIN_SCHEMA_VERSION,
            plugin_id: manifest.id.clone(),
            output: value,
        })
    }

    /// Invokes a command module through bounded in-memory byte streams.
    ///
    /// # Errors
    /// Returns validation, compilation, linking, resource, or execution failures.
    pub async fn invoke_bytes(
        &self,
        manifest: &PluginManifest,
        module_bytes: &[u8],
        input: &[u8],
    ) -> Result<WasiPluginOutput, PluginError> {
        manifest.validate()?;
        if manifest.class != PluginClass::SandboxedWasi {
            return Err(PluginError::Manifest(
                "WASI executor accepts only sandboxed-WASI plugins".into(),
            ));
        }
        if module_bytes.len() > self.limits.maximum_module_bytes {
            return Err(PluginError::ResourceLimit(
                "plugin module is too large".into(),
            ));
        }
        if input.len() > self.limits.maximum_input_bytes {
            return Err(PluginError::ResourceLimit(
                "plugin input is too large".into(),
            ));
        }

        tokio::time::timeout(
            Duration::from_millis(self.limits.maximum_wall_time_ms),
            self.invoke_bounded(manifest, module_bytes, input),
        )
        .await
        .map_err(|_| PluginError::ResourceLimit("plugin wall-time limit was exceeded".into()))?
    }

    async fn invoke_bounded(
        &self,
        manifest: &PluginManifest,
        module_bytes: &[u8],
        input: &[u8],
    ) -> Result<WasiPluginOutput, PluginError> {
        let module = Module::new(&self.engine, module_bytes).map_err(execution_error)?;
        let mut linker = Linker::<WasiStoreState>::new(&self.engine);
        p1::add_to_linker_async(&mut linker, |state| &mut state.wasi).map_err(execution_error)?;
        let pre = linker.instantiate_pre(&module).map_err(execution_error)?;
        let stdout = MemoryOutputPipe::new(self.limits.maximum_output_bytes);
        let stderr = MemoryOutputPipe::new(self.limits.maximum_output_bytes);
        let mut builder = WasiCtxBuilder::new();
        builder
            .stdin(MemoryInputPipe::new(input.to_vec()))
            .stdout(stdout.clone())
            .stderr(stderr.clone())
            .arg(&manifest.id);
        let store_limits = StoreLimitsBuilder::new()
            .memory_size(self.limits.maximum_memory_bytes)
            .table_elements(self.limits.maximum_table_elements)
            .instances(self.limits.maximum_instances)
            .memories(2)
            .tables(2)
            .build();
        let mut store = Store::new(
            &self.engine,
            WasiStoreState {
                wasi: builder.build_p1(),
                limits: store_limits,
            },
        );
        store.limiter(|state| &mut state.limits);
        store.set_fuel(self.limits.fuel).map_err(execution_error)?;
        let instance = pre
            .instantiate_async(&mut store)
            .await
            .map_err(execution_error)?;
        let start = instance
            .get_typed_func::<(), ()>(&mut store, "_start")
            .map_err(execution_error)?;
        start
            .call_async(&mut store, ())
            .await
            .map_err(execution_error)?;
        Ok(WasiPluginOutput {
            stdout: stdout.contents().to_vec(),
            stderr: stderr.contents().to_vec(),
        })
    }
}

fn execution_error(error: impl std::fmt::Display) -> PluginError {
    let detail = format!("{error:#}");
    if detail.contains("fuel")
        || detail.contains("memory")
        || detail.contains("table")
        || detail.contains("capacity")
    {
        PluginError::ResourceLimit(detail)
    } else {
        PluginError::Execution(detail)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn manifest() -> PluginManifest {
        PluginManifest {
            schema_version: PLUGIN_SCHEMA_VERSION,
            id: "example.echo".into(),
            display_name: "Example Echo".into(),
            version: "1.0.0".into(),
            class: PluginClass::SandboxedWasi,
            requested_capabilities: Vec::new(),
        }
    }

    struct NativeEcho;

    impl TrustedNativePlugin for NativeEcho {
        fn manifest(&self) -> PluginManifest {
            PluginManifest {
                class: PluginClass::TrustedNative,
                ..manifest()
            }
        }

        fn invoke(&self, input: &Value) -> Result<Value, PluginError> {
            Ok(input.clone())
        }
    }

    #[test]
    fn trusted_plugins_are_static_validated_and_exactly_routed() {
        let mut registry = TrustedPluginRegistry::new();
        registry.register(Arc::new(NativeEcho)).expect("register");
        assert_eq!(registry.manifests().len(), 1);
        let result = registry
            .invoke(&PluginInvocation {
                schema_version: PLUGIN_SCHEMA_VERSION,
                plugin_id: "example.echo".into(),
                input: json!({"message": "hello"}),
            })
            .expect("invoke");
        assert_eq!(result.output, json!({"message": "hello"}));
        assert!(matches!(
            registry.register(Arc::new(NativeEcho)),
            Err(PluginError::Duplicate(_))
        ));
    }

    #[tokio::test]
    async fn wasi_plugin_writes_one_bounded_json_document() {
        let executor = WasiPluginExecutor::new(WasiPluginLimits::default()).expect("executor");
        let module = br#"
            (module
              (import "wasi_snapshot_preview1" "fd_write"
                (func $fd_write (param i32 i32 i32 i32) (result i32)))
              (memory 1)
              (export "memory" (memory 0))
              (data (i32.const 8) "{\22ok\22:true}")
              (func (export "_start")
                (i32.store (i32.const 0) (i32.const 8))
                (i32.store (i32.const 4) (i32.const 11))
                (drop (call $fd_write
                  (i32.const 1) (i32.const 0) (i32.const 1) (i32.const 20)))))
        "#;
        let result = executor
            .invoke_json(&manifest(), module, &json!({"ignored": true}))
            .await
            .expect("sandboxed invocation");
        assert_eq!(result.output, json!({"ok": true}));
    }

    #[tokio::test]
    async fn wasi_plugin_unknown_import_and_fuel_exhaustion_fail_closed() {
        let limits = WasiPluginLimits {
            fuel: 10_000,
            ..WasiPluginLimits::default()
        };
        let executor = WasiPluginExecutor::new(limits).expect("executor");
        let unknown_import = br#"
            (module
              (import "mcg:host" "ungranted" (func $ungranted))
              (func (export "_start") (call $ungranted)))
        "#;
        assert!(matches!(
            executor
                .invoke_bytes(&manifest(), unknown_import, b"{}")
                .await,
            Err(PluginError::Execution(_))
        ));

        let infinite = br#"(module (func (export "_start") (loop $forever (br $forever))))"#;
        let error = executor
            .invoke_bytes(&manifest(), infinite, b"{}")
            .await
            .expect_err("infinite plugin must exhaust fuel");
        assert!(matches!(error, PluginError::ResourceLimit(_)), "{error:?}");
    }

    #[tokio::test]
    async fn wasi_capability_declarations_never_create_ambient_preopens() {
        let executor = WasiPluginExecutor::new(WasiPluginLimits::default()).expect("executor");
        let mut requested = manifest();
        requested.requested_capabilities = vec![PluginCapability::FilesystemRead];
        let probe = br#"
            (module
              (import "wasi_snapshot_preview1" "fd_prestat_get"
                (func $fd_prestat_get (param i32 i32) (result i32)))
              (import "wasi_snapshot_preview1" "fd_write"
                (func $fd_write (param i32 i32 i32 i32) (result i32)))
              (memory 1)
              (export "memory" (memory 0))
              (data (i32.const 8) "{\22ambient\22:false}")
              (func (export "_start")
                (if (i32.eqz (call $fd_prestat_get (i32.const 3) (i32.const 32)))
                  (then unreachable))
                (i32.store (i32.const 0) (i32.const 8))
                (i32.store (i32.const 4) (i32.const 17))
                (drop (call $fd_write
                  (i32.const 1) (i32.const 0) (i32.const 1) (i32.const 24)))))
        "#;
        let result = executor
            .invoke_json(&requested, probe, &json!({}))
            .await
            .expect("capability declaration does not create a WASI preopen");
        assert_eq!(result.output, json!({"ambient": false}));
    }

    #[tokio::test]
    async fn wasi_plugins_inherit_no_environment_or_filesystem_preopens() {
        let executor = WasiPluginExecutor::new(WasiPluginLimits::default()).expect("executor");
        let module = br#"
            (module
              (import "wasi_snapshot_preview1" "environ_sizes_get"
                (func $environ_sizes_get (param i32 i32) (result i32)))
              (import "wasi_snapshot_preview1" "fd_prestat_get"
                (func $fd_prestat_get (param i32 i32) (result i32)))
              (import "wasi_snapshot_preview1" "fd_write"
                (func $fd_write (param i32 i32 i32 i32) (result i32)))
              (memory 1)
              (export "memory" (memory 0))
              (data (i32.const 16) "{}")
              (func (export "_start")
                (drop (call $environ_sizes_get (i32.const 0) (i32.const 4)))
                (if (i32.load (i32.const 0)) (then unreachable))
                (if (i32.eqz (call $fd_prestat_get (i32.const 3) (i32.const 0)))
                  (then unreachable))
                (i32.store (i32.const 8) (i32.const 16))
                (i32.store (i32.const 12) (i32.const 2))
                (drop (call $fd_write
                  (i32.const 1) (i32.const 8) (i32.const 1) (i32.const 24)))))
        "#;
        let result = executor
            .invoke_json(&manifest(), module, &json!({}))
            .await
            .expect("closed ambient authority");
        assert_eq!(result.output, json!({}));
    }

    #[tokio::test]
    async fn wasi_memory_and_output_limits_fail_closed() {
        let limits = WasiPluginLimits {
            maximum_output_bytes: 4,
            ..WasiPluginLimits::default()
        };
        let executor = WasiPluginExecutor::new(limits).expect("executor");
        let output_overflow = br#"
            (module
              (import "wasi_snapshot_preview1" "fd_write"
                (func $fd_write (param i32 i32 i32 i32) (result i32)))
              (memory 1)
              (export "memory" (memory 0))
              (data (i32.const 8) "0123456789")
              (func (export "_start")
                (i32.store (i32.const 0) (i32.const 8))
                (i32.store (i32.const 4) (i32.const 10))
                (drop (call $fd_write
                  (i32.const 1) (i32.const 0) (i32.const 1) (i32.const 20)))))
        "#;
        assert!(
            executor
                .invoke_json(&manifest(), output_overflow, &json!({}))
                .await
                .is_err()
        );

        let oversized_memory = br#"(module (memory 1025) (func (export "_start")))"#;
        assert!(matches!(
            executor
                .invoke_bytes(&manifest(), oversized_memory, b"{}")
                .await,
            Err(PluginError::ResourceLimit(_))
        ));
    }
}
