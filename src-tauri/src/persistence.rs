mod sqlite;

pub use sqlite::{
    CatalogActivityKind, CatalogIndexWorkerConfig, PersistenceError, PersistentCatalog,
    StoredWorkspace,
};
