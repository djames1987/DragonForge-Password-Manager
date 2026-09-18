use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, ZeroizeOnDrop};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VaultItemKind {
    Login,
    SecureNote,
}

#[derive(Debug, Clone, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
pub struct LoginItem {
    pub username: String,
    pub password: String,
    pub url: String,
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
pub struct SecureNoteItem {
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum VaultItemData {
    Login(LoginItem),
    SecureNote(SecureNoteItem),
}

impl VaultItemData {
    #[must_use]
    pub const fn kind(&self) -> VaultItemKind {
        match self {
            Self::Login(_) => VaultItemKind::Login,
            Self::SecureNote(_) => VaultItemKind::SecureNote,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
pub struct VaultItem {
    pub id: String,
    pub name: String,
    pub favorite: bool,
    pub tags: Vec<String>,
    pub data: VaultItemData,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultItemSummary {
    pub id: String,
    pub name: String,
    pub kind: VaultItemKind,
    pub favorite: bool,
    pub tags: Vec<String>,
    pub updated_at: u64,
}
