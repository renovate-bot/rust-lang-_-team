mod api;

use crate::sync::gws::api::{GoogleWorkspaceApiClient, Group, User};
use std::collections::BTreeSet;
use std::fmt::Debug;

pub(crate) const RUST_LANG_GWS_DOMAIN: &str = "rust-lang.org";

#[allow(dead_code)]
#[derive(Debug, PartialEq)]
pub(crate) enum GoogleGroupDiff {
    Create(Group),
    Delete(Group),
}

#[allow(dead_code)]
#[derive(Debug, PartialEq)]
pub(crate) enum GoogleUserDiff {
    Create(User),
    Delete(User),
}

/// A diff between the team repo and the state on Google Workspace
#[allow(dead_code)]
#[derive(Debug)]
pub(crate) struct GoogleWorkspaceDiff {
    google_groups: Vec<GoogleGroupDiff>,
    google_users: Vec<GoogleUserDiff>,
}

/// The engine that evaluates diffs between our current configuration and
/// the actual state in Google Workspace
#[allow(dead_code)]
pub(crate) struct SyncGoogleWorkspace {
    actual_users: Vec<User>,
    configured_teams: Vec<rust_team_data::v1::Team>,
}

#[allow(dead_code)]
impl SyncGoogleWorkspace {
    pub async fn new(
        teams: Vec<rust_team_data::v1::Team>,
        gws_api_client: Box<dyn GoogleWorkspaceApiClient>,
    ) -> anyhow::Result<Self> {
        let gws_users = gws_api_client.get_users().await?;
        let sync = Self {
            actual_users: gws_users,
            configured_teams: teams,
        };
        Ok(sync)
    }

    pub(crate) fn diff_all(&self) -> anyhow::Result<GoogleWorkspaceDiff> {
        let google_groups_diff = self.diff_groups()?;
        let google_accounts_diff = self.diff_users()?;

        let diff = GoogleWorkspaceDiff {
            google_groups: google_groups_diff,
            google_users: google_accounts_diff,
        };
        Ok(diff)
    }

    fn diff_groups(&self) -> anyhow::Result<Vec<GoogleGroupDiff>> {
        Ok(vec![])
    }

    fn diff_users(&self) -> anyhow::Result<Vec<GoogleUserDiff>> {
        let declared_users = self
            .configured_teams
            .iter()
            .filter(|team| team.google_workspace_saml_group.unwrap_or_default())
            .flat_map(|team| team.members.iter())
            .filter_map(|member| member.google_workspace.as_ref().map(User::from))
            .collect::<BTreeSet<_>>();

        let declared_emails = declared_users
            .iter()
            .map(|user| user.primary_email.as_str())
            .collect::<BTreeSet<_>>();

        let actual_emails = self
            .actual_users
            .iter()
            .map(|user| user.primary_email.as_str())
            .collect::<BTreeSet<_>>();

        let diffs = declared_users
            .iter()
            .filter(|u| !actual_emails.contains(u.primary_email.as_str()))
            .map(|u| GoogleUserDiff::Create(u.clone()))
            .chain(
                self.actual_users
                    .iter()
                    .filter(|u| !declared_emails.contains(u.primary_email.as_str()))
                    .map(|u| GoogleUserDiff::Delete(u.clone())),
            )
            .collect();

        Ok(diffs)
    }
}

#[cfg(test)]
mod tests {
    pub mod rust_team_data_fakes {
        use rust_team_data::v1::{GoogleWorkspace, Team, TeamKind, TeamMember};

        pub fn normal_member(name: &str) -> TeamMember {
            TeamMember {
                name: name.into(),
                github: name.into(),
                github_id: 1234567,
                is_lead: false,
                roles: vec![],
                google_workspace: None,
            }
        }

        pub fn privileged_member(name: &str, surname: &str) -> TeamMember {
            TeamMember {
                google_workspace: Some(GoogleWorkspace {
                    first_name: name.into(),
                    last_name: surname.into(),
                    account_handle: format!("{name}.{surname}"),
                }),
                ..normal_member(name)
            }
        }

        pub fn normal_team(name: &str, members: Vec<TeamMember>) -> Team {
            Team {
                kind: TeamKind::Team,
                name: name.to_string(),
                github: None,
                website_data: None,
                subteam_of: None,
                top_level: Some(true),
                alumni: vec![],
                roles: vec![],
                google_workspace_saml_group: None,
                members,
            }
        }

        pub fn privileged_team(name: &str, members: Vec<TeamMember>) -> Team {
            Team {
                google_workspace_saml_group: Some(true),
                ..normal_team(name, members)
            }
        }
    }

    use crate::sync::gws::api::{GoogleWorkspaceApiClient, User, UserName};
    use crate::sync::gws::tests::rust_team_data_fakes::{privileged_member, privileged_team};
    use crate::sync::gws::{
        GoogleUserDiff, GoogleWorkspaceDiff, RUST_LANG_GWS_DOMAIN, SyncGoogleWorkspace,
    };
    use async_trait::async_trait;
    use rust_team_data::v1::Team;

    struct FakeGoogleWorkspace {
        users: Vec<User>,
    }

    #[async_trait]
    impl GoogleWorkspaceApiClient for FakeGoogleWorkspace {
        async fn get_users(&self) -> anyhow::Result<Vec<User>> {
            Ok(self.users.clone())
        }
    }

    fn google_user(name: &str, surname: &str) -> User {
        User {
            name: UserName {
                given_name: name.into(),
                family_name: surname.into(),
            },
            primary_email: format!("{name}.{surname}@{RUST_LANG_GWS_DOMAIN}"),
        }
    }

    async fn run_sync(
        gws_api_client: Box<dyn GoogleWorkspaceApiClient>,
        teams: Vec<Team>,
    ) -> GoogleWorkspaceDiff {
        let sync = SyncGoogleWorkspace::new(teams, gws_api_client)
            .await
            .expect("cannot create sync");

        let google_users_diff = sync.diff_users().expect("cannot diff accounts");
        let google_groups_diff = sync.diff_groups().expect("cannot diff groups");
        GoogleWorkspaceDiff {
            google_users: google_users_diff,
            google_groups: google_groups_diff,
        }
    }

    fn fake_gws_client(users: Vec<User>) -> Box<dyn GoogleWorkspaceApiClient> {
        let fake_gws = FakeGoogleWorkspace { users };
        Box::new(fake_gws)
    }

    #[tokio::test]
    async fn diff_spots_nothing() {
        let google_users = vec![
            google_user("ubiratan", "soares"),
            google_user("marco", "ieni"),
        ];

        let teams = vec![privileged_team(
            "infra-admins",
            vec![
                privileged_member("ubiratan", "soares"),
                privileged_member("marco", "ieni"),
            ],
        )];

        let gws_api_client = fake_gws_client(google_users);

        let diff = run_sync(gws_api_client, teams).await;
        assert!(diff.google_users.is_empty());
        assert!(diff.google_groups.is_empty());
    }

    #[tokio::test]
    async fn diff_spots_user_creation() {
        let google_users = vec![
            google_user("ubiratan", "soares"),
            google_user("marco", "ieni"),
        ];

        let teams = vec![privileged_team(
            "infra-admins",
            vec![
                privileged_member("ubiratan", "soares"),
                privileged_member("marco", "ieni"),
                privileged_member("emily", "albini"),
            ],
        )];

        let gws_api_client = fake_gws_client(google_users);

        let diff = run_sync(gws_api_client, teams).await;
        let expected = vec![GoogleUserDiff::Create(google_user("emily", "albini"))];

        assert_eq!(diff.google_users, expected);
        assert!(diff.google_groups.is_empty());
    }

    #[tokio::test]
    async fn diff_spots_user_deletion() {
        let google_users = vec![
            google_user("ubiratan", "soares"),
            google_user("marco", "ieni"),
            google_user("emily", "albini"),
        ];

        let teams = vec![privileged_team(
            "infra-admins",
            vec![privileged_member("emily", "albini")],
        )];

        let gws_api_client = fake_gws_client(google_users);

        let diff = run_sync(gws_api_client, teams).await;
        let expected = vec![
            GoogleUserDiff::Delete(google_user("ubiratan", "soares")),
            GoogleUserDiff::Delete(google_user("marco", "ieni")),
        ];

        assert_eq!(diff.google_users, expected);
        assert!(diff.google_groups.is_empty());
    }
}
