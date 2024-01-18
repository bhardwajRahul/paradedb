use async_std::task;
use core::ffi::c_char;
use pgrx::*;

use deltalake::datafusion::catalog::CatalogProvider;
use std::sync::Arc;

use crate::datafusion::context::DatafusionContext;
use crate::datafusion::directory::ParadeDirectory;
use crate::datafusion::schema::ParadeSchemaProvider;
use crate::errors::ParadeError;

#[pg_guard]
#[cfg(any(feature = "pg12", feature = "pg13", feature = "pg14", feature = "pg15"))]
pub extern "C" fn deltalake_relation_set_new_filenode(
    rel: pg_sys::Relation,
    _newrnode: *const pg_sys::RelFileNode,
    persistence: c_char,
    _freezeXid: *mut pg_sys::TransactionId,
    _minmulti: *mut pg_sys::MultiXactId,
) {
    create_table(rel, persistence).expect("Failed to create table");
}

#[pg_guard]
#[cfg(feature = "pg16")]
pub extern "C" fn deltalake_relation_set_new_filelocator(
    rel: pg_sys::Relation,
    _newrlocator: *const pg_sys::RelFileLocator,
    persistence: c_char,
    _freezeXid: *mut pg_sys::TransactionId,
    _minmulti: *mut pg_sys::MultiXactId,
) {
    create_table(rel, persistence).expect("Failed to create table");
}

#[inline]
fn create_table(rel: pg_sys::Relation, persistence: c_char) -> Result<(), ParadeError> {
    let pg_relation = unsafe { PgRelation::from_pg(rel) };

    match persistence as u8 {
        pg_sys::RELPERSISTENCE_UNLOGGED => Err(ParadeError::Generic(
            "Unlogged tables are not yet supported".to_string(),
        )),
        pg_sys::RELPERSISTENCE_TEMP => Err(ParadeError::Generic(
            "Temp tables are not yet supported".to_string(),
        )),
        pg_sys::RELPERSISTENCE_PERMANENT => {
            let schema_name = pg_relation.namespace();
            let schema_oid = pg_relation.namespace_oid();

            DatafusionContext::with_catalog(|catalog| {
                if catalog.schema(schema_name).is_none() {
                    let schema_provider = Arc::new(task::block_on(ParadeSchemaProvider::try_new(
                        schema_name,
                        ParadeDirectory::schema_path(schema_oid)?,
                    ))?);

                    catalog.register_schema(schema_name, schema_provider)?;
                }

                Ok(())
            })?;

            DatafusionContext::with_schema_provider(schema_name, |provider| {
                task::block_on(provider.create_table(&pg_relation))
            })
        }
        _ => Err(ParadeError::Generic("Unknown persistence type".to_string())),
    }
}