// Copyright 2023 - Nym Technologies SA <contact@nymtech.net>
// SPDX-License-Identifier: Apache-2.0

use crate::console_log;
use crate::storage::cipher_export::StoredExportedStoreCipher;
use crate::storage::error::StorageError;
use indexed_db_futures::IdbDatabase;
use nym_store_cipher::{
    Aes256Gcm, Algorithm, EncryptedData, KdfInfo, KeySizeUser, Params, StoreCipher, Unsigned,
    Version,
};
use serde::de::DeserializeOwned;
use serde::Serialize;
use wasm_bindgen::JsValue;

pub use indexed_db_futures::prelude::*;

mod cipher_export;
pub mod error;

pub const CIPHER_INFO_STORE: &str = "_cipher_store";
pub const CIPHER_STORE_EXPORT: &str = "cipher_store_export_info";

const MEMORY_COST: u32 = 19 * 1024;
const ITERATIONS: u32 = 2;
const PARALLELISM: u32 = 1;
const OUTPUT_LENGTH: usize = <Aes256Gcm as KeySizeUser>::KeySize::USIZE;

// use hardcoded values in case any `Default` implementation changes in the future
pub fn new_default_kdf() -> Result<KdfInfo, StorageError> {
    let kdf_salt = KdfInfo::random_salt()?;
    let kdf_info = KdfInfo::Argon2 {
        params: Params::new(MEMORY_COST, ITERATIONS, PARALLELISM, Some(OUTPUT_LENGTH)).unwrap(),
        algorithm: Algorithm::Argon2id,
        version: Version::V0x13,
        kdf_salt,
    };
    Ok(kdf_info)
}

/// An indexeddb-backed in-browser storage with optional encryption.
pub struct WasmStorage {
    inner: IdbWrapper,
    // TODO: this might have to be put behind an Arc.
    store_cipher: Option<StoreCipher>,
}

impl WasmStorage {
    pub async fn new<F>(
        db_name: &str,
        version: u32,
        migrate_fn: Option<F>,
        passphrase: Option<&[u8]>,
    ) -> Result<Self, StorageError>
    where
        F: Fn(&IdbVersionChangeEvent) -> Result<(), JsValue> + 'static,
    {
        let mut db_req: OpenDbRequest = IdbDatabase::open_u32(db_name, version)?;

        // we must always ensure the cipher table is present
        db_req.set_on_upgrade_needed(Some(
            move |evt: &IdbVersionChangeEvent| -> Result<(), JsValue> {
                // Even if the web-sys bindings expose the version as a f64, the IndexedDB API
                // works with an unsigned integer.
                // See <https://github.com/rustwasm/wasm-bindgen/issues/1149>
                let old_version = evt.old_version() as u32;

                if old_version < 1 {
                    evt.db().create_object_store(CIPHER_INFO_STORE)?;
                }

                if let Some(migrate) = migrate_fn.as_ref() {
                    migrate(evt)
                } else {
                    Ok(())
                }
            },
        ));

        let db: IdbDatabase = db_req.into_future().await?;
        let inner = IdbWrapper(db);
        let store_cipher = inner.setup_store_cipher(passphrase).await?;

        Ok(WasmStorage {
            inner,
            store_cipher,
        })
    }

    pub fn serialize_value<T: Serialize>(&self, value: &T) -> Result<JsValue, StorageError> {
        if let Some(cipher) = &self.store_cipher {
            let encrypted = cipher.encrypt_json_value(value)?;
            Ok(serde_wasm_bindgen::to_value(&encrypted)?)
        } else {
            Ok(serde_wasm_bindgen::to_value(&value)?)
        }
    }

    pub fn deserialize_value<T: DeserializeOwned>(
        &self,
        value: JsValue,
    ) -> Result<T, StorageError> {
        if let Some(cipher) = &self.store_cipher {
            let encrypted: EncryptedData = serde_wasm_bindgen::from_value(value)?;
            Ok(cipher.decrypt_json_value(encrypted)?)
        } else {
            Ok(serde_wasm_bindgen::from_value(value)?)
        }
    }

    pub async fn read_value<T, K>(&self, store: &str, key: K) -> Result<Option<T>, StorageError>
    where
        T: DeserializeOwned,
        K: wasm_bindgen::JsCast,
    {
        self.inner
            .read_value_raw(store, key)
            .await?
            .map(|raw| self.deserialize_value(raw))
            .transpose()
    }

    pub async fn store_value<T, K>(
        &self,
        store: &str,
        key: K,
        value: &T,
    ) -> Result<(), StorageError>
    where
        T: Serialize,
        K: wasm_bindgen::JsCast,
    {
        self.inner
            .store_value_raw(store, key, &self.serialize_value(&value)?)
            .await
    }
}

struct IdbWrapper(IdbDatabase);

impl IdbWrapper {
    async fn read_value_raw<K>(&self, store: &str, key: K) -> Result<Option<JsValue>, StorageError>
    where
        K: wasm_bindgen::JsCast,
    {
        self.0
            .transaction_on_one_with_mode(store, IdbTransactionMode::Readonly)?
            .object_store(store)?
            .get(&key)?
            .await
            .map_err(Into::into)
    }

    async fn store_value_raw<K>(
        &self,
        store: &str,
        key: K,
        value: &JsValue,
    ) -> Result<(), StorageError>
    where
        K: wasm_bindgen::JsCast,
    {
        self.0
            .transaction_on_one_with_mode(store, IdbTransactionMode::Readwrite)?
            .object_store(store)?
            .put_key_val_owned(key, value)?
            .into_future()
            .await
            .map_err(Into::into)
    }

    async fn read_exported_cipher_store(
        &self,
    ) -> Result<Option<StoredExportedStoreCipher>, StorageError> {
        self.read_value_raw(CIPHER_INFO_STORE, JsValue::from_str(CIPHER_STORE_EXPORT))
            .await?
            .map(serde_wasm_bindgen::from_value)
            .transpose()
            .map_err(Into::into)
    }

    async fn store_exported_cipher_store(
        &self,
        exported_store_cipher: StoredExportedStoreCipher,
    ) -> Result<(), StorageError> {
        self.store_value_raw(
            CIPHER_INFO_STORE,
            JsValue::from_str(CIPHER_STORE_EXPORT),
            &serde_wasm_bindgen::to_value(&exported_store_cipher)?,
        )
        .await
    }

    async fn setup_new_store_cipher(
        &self,
        passphrase: Option<&[u8]>,
    ) -> Result<Option<StoreCipher>, StorageError> {
        if let Some(passphrase) = passphrase {
            console_log!("attempting to derive new encryption key");
            let kdf_info = new_default_kdf()?;
            let store_cipher = StoreCipher::<Aes256Gcm>::new(passphrase, kdf_info)?;
            let exported = store_cipher.export_aes256gcm()?;
            self.store_exported_cipher_store(Some(exported).into())
                .await?;

            Ok(Some(store_cipher))
        } else {
            console_log!("this new storage will not use any encryption");
            self.store_exported_cipher_store(StoredExportedStoreCipher::NoEncryption)
                .await?;
            Ok(None)
        }
    }

    async fn restore_existing_cipher(
        &self,
        existing: StoredExportedStoreCipher,
        passphrase: Option<&[u8]>,
    ) -> Result<Option<StoreCipher>, StorageError> {
        if let Some(passphrase) = passphrase {
            console_log!("attempting to use previously derived encryption key");
            if let StoredExportedStoreCipher::Cipher(exported_cipher) = existing {
                Ok(Some(StoreCipher::import_aes256gcm(
                    passphrase,
                    exported_cipher,
                )?))
            } else {
                Err(StorageError::UnexpectedPassphraseProvided)
            }
        } else {
            console_log!("attempting to restore old unencrypted data");
            if existing.uses_encryption() {
                Err(StorageError::NoPassphraseProvided)
            } else {
                Ok(None)
            }
        }
    }

    async fn setup_store_cipher(
        &self,
        passphrase: Option<&[u8]>,
    ) -> Result<Option<StoreCipher>, StorageError> {
        // we have few options of proceeding from here:
        // no passphrase + no existing info => it's a fresh client that won't use encryption, so just store that info
        // no passphrase + existing info => check if the existing info has kdf details, if so, reject
        // passphrase + no existing info => it's a fresh client that will use encryption, so derive what's required and store it
        // passphrase + existing info => check if the existing info has kdf details, if so, try to re-derive the key

        if let Some(existing_cipher_info) = self.read_exported_cipher_store().await? {
            self.restore_existing_cipher(existing_cipher_info, passphrase)
                .await
        } else {
            self.setup_new_store_cipher(passphrase).await
        }
    }
}
