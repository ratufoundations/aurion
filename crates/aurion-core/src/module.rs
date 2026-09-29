#![forbid(unsafe_code)]

use crate::State;
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use thiserror::Error;

/// Informasi deterministik yang tersedia saat sebuah command modul dieksekusi.
#[derive(Debug, Clone, Copy)]
pub struct ExecutionContext<'a> {
    pub height: u64,
    pub timestamp: u64,
    pub caller: &'a [u8; 32],
}

/// Antarmuka baca state. Kunci yang terlihat modul sudah dibatasi ke namespace miliknya.
pub trait StateReader {
    fn get(&self, key: &[u8]) -> Option<&[u8]>;
}

/// Antarmuka perubahan state untuk hook genesis dan command modul.
pub trait StateWriter: StateReader {
    fn set(&mut self, key: &[u8], value: &[u8]);
    fn remove(&mut self, key: &[u8]) -> Option<Vec<u8>>;
}

/// Kontrak object-safe untuk modul yang dapat dipasang pada runtime Aurion.
pub trait AurionModule: Send + Sync {
    fn module_id(&self) -> &'static [u8];

    /// Inisialisasi state modul pada genesis.
    ///
    /// # Errors
    /// Mengembalikan error jika inisialisasi modul gagal.
    fn init_genesis(&self, state: &mut dyn StateWriter)
        -> Result<(), Box<dyn Error + Send + Sync>>;

    /// Menjalankan satu transisi state deterministik.
    ///
    /// # Errors
    /// Mengembalikan error jika aksi atau payload tidak valid.
    fn execute(
        &self,
        ctx: &ExecutionContext<'_>,
        action: u16,
        payload: &[u8],
        state: &mut dyn StateWriter,
    ) -> Result<(), Box<dyn Error + Send + Sync>>;

    /// Membaca proyeksi data modul.
    ///
    /// # Errors
    /// Mengembalikan error jika jalur kueri tidak dikenal atau pembacaan gagal.
    fn query(
        &self,
        path: &str,
        state: &dyn StateReader,
    ) -> Result<Vec<u8>, Box<dyn Error + Send + Sync>>;
}

#[derive(Error, Debug)]
pub enum DispatchError {
    #[error("ID modul tidak boleh kosong")]
    EmptyModuleId,
    #[error("Modul dengan ID {0:?} sudah terdaftar")]
    DuplicateModule(Vec<u8>),
    #[error("Modul dengan ID {0:?} tidak ditemukan")]
    ModuleNotFound(Vec<u8>),
    #[error("Hook genesis modul {module_id:?} gagal: {source}")]
    GenesisFailed {
        module_id: Vec<u8>,
        #[source]
        source: Box<dyn Error + Send + Sync>,
    },
    #[error("Eksekusi modul {module_id:?} gagal: {source}")]
    ExecutionFailed {
        module_id: Vec<u8>,
        #[source]
        source: Box<dyn Error + Send + Sync>,
    },
    #[error("Kueri modul {module_id:?} gagal: {source}")]
    QueryFailed {
        module_id: Vec<u8>,
        #[source]
        source: Box<dyn Error + Send + Sync>,
    },
}

/// Registry modul terurut yang membuat urutan hook genesis tetap deterministik.
#[derive(Default)]
pub struct ModuleDispatcher {
    modules: BTreeMap<Vec<u8>, Box<dyn AurionModule>>,
}

impl fmt::Debug for ModuleDispatcher {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ModuleDispatcher")
            .field("module_ids", &self.modules.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl ModuleDispatcher {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Memasang modul dan menolak ID kosong maupun pendaftaran ganda.
    ///
    /// # Errors
    /// Mengembalikan `EmptyModuleId` atau `DuplicateModule` jika ID tidak dapat dipasang.
    pub fn register_module(&mut self, module: Box<dyn AurionModule>) -> Result<(), DispatchError> {
        let module_id = module.module_id();
        if module_id.is_empty() {
            return Err(DispatchError::EmptyModuleId);
        }
        if self.modules.contains_key(module_id) {
            return Err(DispatchError::DuplicateModule(module_id.to_vec()));
        }
        self.modules.insert(module_id.to_vec(), module);
        Ok(())
    }

    pub fn module_ids(&self) -> impl Iterator<Item = &[u8]> {
        self.modules.keys().map(Vec::as_slice)
    }

    /// Menjalankan hook genesis berurutan berdasarkan ID modul secara atomik.
    ///
    /// # Errors
    /// Mengembalikan `GenesisFailed` dan membuang perubahan sementara bila hook gagal.
    pub fn init_genesis(&self, state: &mut State) -> Result<(), DispatchError> {
        let mut staged = state.clone();
        for (module_id, module) in &self.modules {
            let mut scoped = NamespacedWriter::new(&mut staged, module_id);
            module
                .init_genesis(&mut scoped)
                .map_err(|source| DispatchError::GenesisFailed {
                    module_id: module_id.clone(),
                    source,
                })?;
        }
        *state = staged;
        Ok(())
    }

    /// Mengarahkan aksi ke modul dan menyimpan mutasi hanya jika aksi berhasil.
    ///
    /// # Errors
    /// Mengembalikan `ModuleNotFound` atau `ExecutionFailed`; kegagalan tidak mengubah state.
    pub fn dispatch(
        &self,
        target_module: &[u8],
        ctx: &ExecutionContext<'_>,
        action: u16,
        payload: &[u8],
        state: &mut State,
    ) -> Result<(), DispatchError> {
        let module = self
            .modules
            .get(target_module)
            .ok_or_else(|| DispatchError::ModuleNotFound(target_module.to_vec()))?;
        let mut staged = state.clone();
        let mut scoped = NamespacedWriter::new(&mut staged, target_module);
        module
            .execute(ctx, action, payload, &mut scoped)
            .map_err(|source| DispatchError::ExecutionFailed {
                module_id: target_module.to_vec(),
                source,
            })?;
        *state = staged;
        Ok(())
    }

    /// Menjalankan kueri pada state namespace modul.
    ///
    /// # Errors
    /// Mengembalikan `ModuleNotFound` atau `QueryFailed` jika modul/kueri gagal.
    pub fn query(
        &self,
        target_module: &[u8],
        path: &str,
        state: &State,
    ) -> Result<Vec<u8>, DispatchError> {
        let module = self
            .modules
            .get(target_module)
            .ok_or_else(|| DispatchError::ModuleNotFound(target_module.to_vec()))?;
        let scoped = NamespacedReader::new(state, target_module);
        module
            .query(path, &scoped)
            .map_err(|source| DispatchError::QueryFailed {
                module_id: target_module.to_vec(),
                source,
            })
    }
}

/// Encode the namespace length before its bytes so module/key pairs cannot collide.
pub(crate) fn namespaced_key(module_id: &[u8], key: &[u8]) -> Option<Vec<u8>> {
    let namespace_len = u64::try_from(module_id.len()).ok()?;
    let mut namespaced = Vec::with_capacity(8 + module_id.len() + key.len());
    namespaced.extend_from_slice(&namespace_len.to_be_bytes());
    namespaced.extend_from_slice(module_id);
    namespaced.extend_from_slice(key);
    Some(namespaced)
}

struct NamespacedReader<'a> {
    state: &'a dyn StateReader,
    module_id: &'a [u8],
}

impl<'a> NamespacedReader<'a> {
    fn new(state: &'a dyn StateReader, module_id: &'a [u8]) -> Self {
        Self { state, module_id }
    }
}

impl StateReader for NamespacedReader<'_> {
    fn get(&self, key: &[u8]) -> Option<&[u8]> {
        namespaced_key(self.module_id, key).and_then(|namespaced| self.state.get(&namespaced))
    }
}

struct NamespacedWriter<'a> {
    state: &'a mut dyn StateWriter,
    module_id: &'a [u8],
}

impl<'a> NamespacedWriter<'a> {
    fn new(state: &'a mut dyn StateWriter, module_id: &'a [u8]) -> Self {
        Self { state, module_id }
    }
}

impl StateReader for NamespacedWriter<'_> {
    fn get(&self, key: &[u8]) -> Option<&[u8]> {
        namespaced_key(self.module_id, key).and_then(|namespaced| self.state.get(&namespaced))
    }
}

impl StateWriter for NamespacedWriter<'_> {
    fn set(&mut self, key: &[u8], value: &[u8]) {
        if let Some(namespaced) = namespaced_key(self.module_id, key) {
            self.state.set(&namespaced, value);
        }
    }

    fn remove(&mut self, key: &[u8]) -> Option<Vec<u8>> {
        namespaced_key(self.module_id, key).and_then(|namespaced| self.state.remove(&namespaced))
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::State;

    struct CounterModule {
        id: &'static [u8],
    }

    impl AurionModule for CounterModule {
        fn module_id(&self) -> &'static [u8] {
            self.id
        }

        fn init_genesis(
            &self,
            state: &mut dyn StateWriter,
        ) -> Result<(), Box<dyn Error + Send + Sync>> {
            state.set(b"count", &0u64.to_le_bytes());
            Ok(())
        }

        fn execute(
            &self,
            _ctx: &ExecutionContext<'_>,
            _action: u16,
            _payload: &[u8],
            state: &mut dyn StateWriter,
        ) -> Result<(), Box<dyn Error + Send + Sync>> {
            let current = state
                .get(b"count")
                .and_then(|bytes| bytes.try_into().ok())
                .map_or(0, u64::from_le_bytes);
            state.set(b"count", &current.saturating_add(1).to_le_bytes());
            Ok(())
        }

        fn query(
            &self,
            _path: &str,
            state: &dyn StateReader,
        ) -> Result<Vec<u8>, Box<dyn Error + Send + Sync>> {
            Ok(state.get(b"count").unwrap_or_default().to_vec())
        }
    }

    #[test]
    fn dispatcher_isolates_namespaces_and_routes_commands_and_queries() {
        let mut dispatcher = ModuleDispatcher::new();
        dispatcher
            .register_module(Box::new(CounterModule { id: b"counter-a" }))
            .expect("register module");
        dispatcher
            .register_module(Box::new(CounterModule { id: b"counter-b" }))
            .expect("register module");

        let mut state = State::new();
        dispatcher
            .init_genesis(&mut state)
            .expect("initialize genesis");
        let caller = [7u8; 32];
        let context = ExecutionContext {
            height: 1,
            timestamp: 42,
            caller: &caller,
        };
        dispatcher
            .dispatch(b"counter-a", &context, 1, &[], &mut state)
            .expect("dispatch module command");

        assert_eq!(
            dispatcher
                .query(b"counter-a", "count", &state)
                .expect("query module"),
            1u64.to_le_bytes()
        );
        assert_eq!(
            dispatcher
                .query(b"counter-b", "count", &state)
                .expect("query other module"),
            0u64.to_le_bytes()
        );
    }

    #[test]
    fn dispatcher_rejects_duplicate_and_unknown_modules() {
        let mut dispatcher = ModuleDispatcher::new();
        dispatcher
            .register_module(Box::new(CounterModule { id: b"counter" }))
            .expect("register module");
        assert!(matches!(
            dispatcher.register_module(Box::new(CounterModule { id: b"counter" })),
            Err(DispatchError::DuplicateModule(_))
        ));
        assert!(matches!(
            dispatcher.query(b"unknown", "", &State::new()),
            Err(DispatchError::ModuleNotFound(_))
        ));
    }
}
