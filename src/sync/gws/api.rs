use crate::sync::gws::RUST_LANG_GWS_DOMAIN;
use async_trait::async_trait;
use rust_team_data::v1::GoogleWorkspace;

/// https://developers.google.com/workspace/admin/directory/reference/rest/v1/groups
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Group {
    pub name: String,
    pub email: String,
}

/// https://developers.google.com/workspace/admin/directory/reference/rest/v1/users#UserName
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct UserName {
    pub given_name: String,
    pub family_name: String,
}

/// https://developers.google.com/workspace/admin/directory/reference/rest/v1/users
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct User {
    pub name: UserName,
    pub primary_email: String,
}

impl From<&GoogleWorkspace> for User {
    fn from(gws: &GoogleWorkspace) -> Self {
        Self {
            primary_email: format!("{}@{}", gws.account_handle, RUST_LANG_GWS_DOMAIN),
            name: UserName {
                given_name: gws.first_name.to_string(),
                family_name: gws.last_name.to_string(),
            },
        }
    }
}

#[async_trait]
pub(crate) trait GoogleWorkspaceApiClient {
    async fn get_users(&self) -> anyhow::Result<Vec<User>>;
}
