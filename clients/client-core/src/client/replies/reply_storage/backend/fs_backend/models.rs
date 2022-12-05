// Copyright 2022 - Nym Technologies SA <contact@nymtech.net>
// SPDX-License-Identifier: Apache-2.0

use crate::client::replies::reply_storage::backend::fs_backend::error::StorageError;
use crypto::generic_array::typenum::Unsigned;
use crypto::Digest;
use nymsphinx::addressing::clients::{Recipient, RecipientBytes};
use nymsphinx::anonymous_replies::encryption_key::EncryptionKeyDigest;
use nymsphinx::anonymous_replies::requests::{AnonymousSenderTag, SENDER_TAG_SIZE};
use nymsphinx::anonymous_replies::{ReplySurb, SurbEncryptionKey, SurbEncryptionKeySize};
use nymsphinx::params::ReplySurbKeyDigestAlgorithm;
use std::time::Duration;
use time::OffsetDateTime;
use tokio::time::Instant;

#[derive(Debug, Clone)]
pub(crate) struct StoredSenderTag {
    pub(crate) recipient: Vec<u8>,
    pub(crate) tag: Vec<u8>,
}

impl StoredSenderTag {
    pub(crate) fn new(recipient: RecipientBytes, tag: AnonymousSenderTag) -> StoredSenderTag {
        StoredSenderTag {
            recipient: recipient.to_vec(),
            tag: tag.to_bytes().to_vec(),
        }
    }
}

impl TryFrom<StoredSenderTag> for (RecipientBytes, AnonymousSenderTag) {
    type Error = StorageError;

    fn try_from(value: StoredSenderTag) -> Result<Self, Self::Error> {
        let recipient_len = value.recipient.len();
        let Ok(recipient_bytes) = value.recipient.try_into() else {
            return Err(StorageError::CorruptedData {
                details: format!(
                    "the retrieved recipient has length of {recipient_len} while {} was expected",
                    Recipient::LEN
                ),
            });
        };

        let tag_len = value.tag.len();
        let Ok(sender_tag_bytes) = value.tag.try_into() else {
            return Err(StorageError::CorruptedData {
                details: format!(
                    "the retrieved sender tag has length of {tag_len} while {} was expected",
                    SENDER_TAG_SIZE
                ),
            });
        };

        Ok((
            recipient_bytes,
            AnonymousSenderTag::from_bytes(sender_tag_bytes),
        ))
    }
}

#[derive(Debug, Clone)]
pub(crate) struct StoredReplyKey {
    pub(crate) key_digest: Vec<u8>,
    pub(crate) reply_key: Vec<u8>,
}

impl StoredReplyKey {
    pub(crate) fn new(
        key_digest: EncryptionKeyDigest,
        reply_key: SurbEncryptionKey,
    ) -> StoredReplyKey {
        StoredReplyKey {
            key_digest: key_digest.to_vec(),
            reply_key: reply_key.to_bytes(),
        }
    }
}

impl TryFrom<StoredReplyKey> for (EncryptionKeyDigest, SurbEncryptionKey) {
    type Error = StorageError;

    fn try_from(value: StoredReplyKey) -> Result<Self, Self::Error> {
        let expected_reply_key_digest_size = ReplySurbKeyDigestAlgorithm::output_size();
        let reply_key_digest_size = value.key_digest.len();

        let Some(digest) = EncryptionKeyDigest::from_exact_iter(value.key_digest) else {
            return Err(StorageError::CorruptedData {
                details: format!(
                    "the reply surb digest has length of {reply_key_digest_size} while {expected_reply_key_digest_size} was expected",
                ),
            });
        };

        let reply_key_len = value.reply_key.len();
        let Ok(reply_key) = SurbEncryptionKey::try_from_bytes(&value.reply_key) else {
            return Err(StorageError::CorruptedData {
                details: format!(
                    "the reply key has length of {reply_key_len} while {} was expected",
                    SurbEncryptionKeySize::USIZE
                ),
            });
        };

        Ok((digest, reply_key))
    }
}

pub(crate) struct StoredSurbSender {
    pub(crate) id: i64,
    pub(crate) tag: Vec<u8>,
    pub(crate) last_sent_timestamp: i64,
}

impl StoredSurbSender {
    pub(crate) fn new(tag: AnonymousSenderTag, last_sent: Instant) -> Self {
        // this doesn't have to be sub-second accurate
        // as a matter of fact even if it's off by few minutes or even hours,
        // it would still be good enough
        let elapsed = last_sent.elapsed();
        let now = OffsetDateTime::now_utc();
        let last_sent = now - elapsed;

        StoredSurbSender {
            // for the purposes of STORING data,
            // we ignore that field anyway
            id: 0,
            tag: tag.to_bytes().to_vec(),
            last_sent_timestamp: last_sent.unix_timestamp(),
        }
    }
}

impl TryFrom<StoredSurbSender> for (AnonymousSenderTag, Instant) {
    type Error = StorageError;

    fn try_from(value: StoredSurbSender) -> Result<Self, Self::Error> {
        let tag_len = value.tag.len();
        let Ok(sender_tag_bytes) = value.tag.try_into() else {
            return Err(StorageError::CorruptedData {
                details: format!(
                    "the retrieved sender tag has length of {tag_len} while {} was expected",
                    SENDER_TAG_SIZE
                ),
            });
        };

        let datetime =
            OffsetDateTime::from_unix_timestamp(value.last_sent_timestamp).map_err(|err| {
                StorageError::CorruptedData {
                    details: format!("failed to parse stored timestamp - {err}"),
                }
            })?;

        let duration_since: Duration =
            (OffsetDateTime::now_utc() - datetime)
                .try_into()
                .map_err(|err| StorageError::CorruptedData {
                    details: format!(
                        "failed to extract valid duration from the stored timestamp - {err}"
                    ),
                })?;

        let now = Instant::now();
        let instant = now.checked_sub(duration_since).unwrap_or(now);

        Ok((AnonymousSenderTag::from_bytes(sender_tag_bytes), instant))
    }
}

pub(crate) struct StoredReplySurb {
    pub(crate) reply_surb_sender_id: i64,
    pub(crate) reply_surb: Vec<u8>,
}

impl StoredReplySurb {
    pub(crate) fn new(reply_surb_sender_id: i64, reply_surb: &ReplySurb) -> Self {
        StoredReplySurb {
            reply_surb_sender_id,
            reply_surb: reply_surb.to_bytes(),
        }
    }
}

impl TryFrom<StoredReplySurb> for ReplySurb {
    type Error = StorageError;

    fn try_from(value: StoredReplySurb) -> Result<Self, Self::Error> {
        ReplySurb::from_bytes(&value.reply_surb).map_err(|err| StorageError::CorruptedData {
            details: format!("failed to recover the reply surb: {err}"),
        })
    }
}

#[derive(Copy, Clone)]
pub(crate) struct ReplySurbStorageMetadata {
    pub(crate) min_reply_surb_threshold: u32,
    pub(crate) max_reply_surb_threshold: u32,
}

impl ReplySurbStorageMetadata {
    pub(crate) fn new(min_reply_surb_threshold: usize, max_reply_surb_threshold: usize) -> Self {
        Self {
            min_reply_surb_threshold: min_reply_surb_threshold as u32,
            max_reply_surb_threshold: max_reply_surb_threshold as u32,
        }
    }
}
