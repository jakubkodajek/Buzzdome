//! The quizmaster's remote: what each of its keys does in a show. The
//! receiver hears the keys (`buzzers::RemoteKey`), the receiver's settings
//! keep the quizmaster's choices (`hardware`), and the Director carries a
//! key's action out as it does the same action's key on the keyboard
//! (`keyboard.ts`), one step per press.

use buzzers::RemoteKey;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use ts_rs::TS;

/// What a key of the remote does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum RemoteAction {
    /// Nothing, so a key pressed by mistake does no harm.
    Nothing,
    Next,
    /// Back to the slide before, between questions (`Command::GoTo`).
    Previous,
    /// Pause the question clock, or go on with it.
    Clock,
    CloseQuestion,
    JudgeRight,
    JudgeWrong,
    /// Show the leaderboard, or hide it.
    Leaderboard,
    PlayVideo,
    /// Open the projector window, or close it.
    Projector,
}

/// What `key` does until the quizmaster chooses otherwise: the arrows go
/// back and on, the marks judge, and the keys whose picture says nothing
/// in a show do nothing.
fn default_action(key: RemoteKey) -> RemoteAction {
    match key {
        RemoteKey::Back => RemoteAction::Previous,
        RemoteKey::Forward | RemoteKey::Centre => RemoteAction::Next,
        RemoteKey::Target => RemoteAction::CloseQuestion,
        RemoteKey::PlayPause => RemoteAction::Clock,
        RemoteKey::Trophy => RemoteAction::Leaderboard,
        RemoteKey::ColourBars => RemoteAction::PlayVideo,
        RemoteKey::Cross => RemoteAction::JudgeWrong,
        RemoteKey::Tick => RemoteAction::JudgeRight,
        RemoteKey::Power | RemoteKey::Grid | RemoteKey::Question => RemoteAction::Nothing,
    }
}

/// A key of the remote with what it does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct RemoteKeyAction {
    pub key: RemoteKey,
    pub action: RemoteAction,
}

/// What the quizmaster chose for the keys whose action is not the default.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RemoteKeys(BTreeMap<RemoteKey, RemoteAction>);

impl RemoteKeys {
    pub fn action(&self, key: RemoteKey) -> RemoteAction {
        self.0.get(&key).copied().unwrap_or_else(|| default_action(key))
    }

    /// Makes `key` do `action`; one set back to its default is kept as such.
    pub fn set(&mut self, key: RemoteKey, action: RemoteAction) {
        if action == default_action(key) {
            self.0.remove(&key);
        } else {
            self.0.insert(key, action);
        }
    }

    /// Every key with what it does, in the order of the remote's codes.
    pub fn all(&self) -> Vec<RemoteKeyAction> {
        RemoteKey::ALL.into_iter().map(|key| RemoteKeyAction { key, action: self.action(key) }).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_does_its_default_until_chosen_otherwise() {
        let mut keys = RemoteKeys::default();
        assert_eq!(keys.action(RemoteKey::Forward), RemoteAction::Next);
        assert_eq!(keys.action(RemoteKey::Power), RemoteAction::Nothing, "no key turns the projector off by mistake");
        keys.set(RemoteKey::Power, RemoteAction::Projector);
        keys.set(RemoteKey::Forward, RemoteAction::Next);
        assert_eq!(keys.action(RemoteKey::Power), RemoteAction::Projector);
        assert_eq!(serde_json::to_string(&keys).unwrap(), r#"{"power":"projector"}"#, "only the choices are kept");
        keys.set(RemoteKey::Power, RemoteAction::Nothing);
        assert_eq!(keys, RemoteKeys::default());
    }

    #[test]
    fn every_key_is_listed_once_and_the_defaults_judge_and_step() {
        let all = RemoteKeys::default().all();
        assert_eq!(all.iter().map(|k| k.key).collect::<Vec<_>>(), RemoteKey::ALL);
        let action = |key| all.iter().find(|k| k.key == key).unwrap().action;
        assert_eq!((action(RemoteKey::Tick), action(RemoteKey::Cross)), (RemoteAction::JudgeRight, RemoteAction::JudgeWrong));
        assert_eq!((action(RemoteKey::Back), action(RemoteKey::PlayPause)), (RemoteAction::Previous, RemoteAction::Clock));
    }
}
