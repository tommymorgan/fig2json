//! Targeting a single node within a built document tree.
//!
//! A node's identity is its `guid` `{sessionID, localID}`. Users pass the id as
//! copied from Figma: REST form uses a colon (`6886:48774`), a figma.com URL uses
//! a dash (`?node-id=6886-48774`). Both separators are accepted; the first number
//! is matched against `sessionID`, the second against `localID`. `find_node`
//! returns the matching node together with its subtree, so callers can scope the
//! whole conversion to one frame.

use std::fmt;
use std::str::FromStr;

use serde_json::Value as JsonValue;

/// A Figma node identifier, matching a `guid` `{sessionID, localID}`. Both halves
/// are unsigned, like the values the `.fig` decoder produces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeId {
    pub session_id: u64,
    pub local_id: u64,
}

impl fmt::Display for NodeId {
    /// Renders as `sessionID:localID`, the REST form.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.session_id, self.local_id)
    }
}

impl FromStr for NodeId {
    type Err = String;

    /// Accepts `sessionID:localID` (REST) or `sessionID-localID` (URL), with
    /// optional surrounding whitespace around each half.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let invalid = || format!("expected sessionID:localID, e.g. 6886:48774, got {s:?}");
        let (session, local) = s.split_once([':', '-']).ok_or_else(invalid)?;
        Ok(NodeId {
            session_id: session.trim().parse().map_err(|_| invalid())?,
            local_id: local.trim().parse().map_err(|_| invalid())?,
        })
    }
}

/// Parse a node id in REST form `sessionID:localID` or URL form `sessionID-localID`,
/// returning `None` if it does not parse.
pub fn parse_node_id(s: &str) -> Option<NodeId> {
    s.parse().ok()
}

/// Find the node whose `guid` matches `id`, returning a clone of that node and
/// its subtree. Searches depth-first and returns the first match (guids are
/// unique within a document); returns `None` when no node matches.
pub(crate) fn find_node(value: &JsonValue, id: &NodeId) -> Option<JsonValue> {
    match value {
        JsonValue::Object(map) => {
            if map.get("guid").is_some_and(|guid| guid_matches(guid, id)) {
                return Some(value.clone());
            }
            map.values().find_map(|child| find_node(child, id))
        }
        JsonValue::Array(items) => items.iter().find_map(|child| find_node(child, id)),
        _ => None,
    }
}

fn guid_matches(guid: &JsonValue, id: &NodeId) -> bool {
    guid.get("localID").and_then(JsonValue::as_u64) == Some(id.local_id)
        && guid.get("sessionID").and_then(JsonValue::as_u64) == Some(id.session_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn id(session_id: u64, local_id: u64) -> NodeId {
        NodeId { session_id, local_id }
    }

    #[test]
    fn parses_rest_colon_form() {
        assert_eq!(parse_node_id("6886:48774"), Some(id(6886, 48774)));
    }

    #[test]
    fn parses_url_dash_form() {
        assert_eq!(parse_node_id("6886-48774"), Some(id(6886, 48774)));
    }

    #[test]
    fn accepts_surrounding_whitespace() {
        assert_eq!(parse_node_id("  6886 : 48774 "), Some(id(6886, 48774)));
    }

    #[test]
    fn rejects_unparseable_id() {
        assert_eq!(parse_node_id("not-a-pair-of-ints"), None);
        assert_eq!(parse_node_id("6886"), None);
    }

    #[test]
    fn rejects_missing_half() {
        assert_eq!(parse_node_id("6886:"), None);
        assert_eq!(parse_node_id(":48774"), None);
    }

    #[test]
    fn rejects_negative_id() {
        // Figma session/local ids are unsigned; a negative can never match a node,
        // so it should be rejected at parse time rather than become a phantom miss.
        assert_eq!(parse_node_id("5:-3"), None);
    }

    #[test]
    fn from_str_error_names_the_expected_form() {
        let err = "garbage".parse::<NodeId>().unwrap_err();
        assert!(err.contains("sessionID:localID"), "unhelpful error: {err}");
    }

    #[test]
    fn displays_as_session_colon_local() {
        assert_eq!(id(6886, 48774).to_string(), "6886:48774");
    }

    #[test]
    fn finds_nested_node_with_its_subtree_excluding_siblings() {
        let tree = json!({
            "document": {
                "guid": { "localID": 0, "sessionID": 0 },
                "children": [
                    {
                        "guid": { "localID": 48774, "sessionID": 6886 },
                        "name": "Target",
                        "children": [
                            { "guid": { "localID": 1, "sessionID": 6886 }, "name": "Kid" }
                        ]
                    },
                    { "guid": { "localID": 2, "sessionID": 6886 }, "name": "Sibling" }
                ]
            }
        });

        let found = find_node(&tree, &id(6886, 48774)).unwrap();

        assert_eq!(found["name"], "Target");
        // The returned subtree carries the node's own children and excludes siblings.
        let children = found["children"].as_array().unwrap();
        assert_eq!(children.len(), 1, "subtree must not include siblings");
        assert_eq!(children[0]["name"], "Kid");
    }

    #[test]
    fn returns_first_match_in_depth_first_order() {
        let tree = json!({
            "children": [
                { "guid": { "localID": 7, "sessionID": 1 }, "name": "First" },
                { "guid": { "localID": 7, "sessionID": 1 }, "name": "Second" }
            ]
        });

        let found = find_node(&tree, &id(1, 7)).unwrap();

        assert_eq!(found["name"], "First");
    }

    #[test]
    fn returns_none_when_no_node_matches() {
        let tree = json!({ "guid": { "localID": 0, "sessionID": 0 } });
        assert!(find_node(&tree, &id(9, 9)).is_none());
    }

    #[test]
    fn ignores_matching_ids_that_are_not_node_guids() {
        // At scoping time the tree still carries {sessionID, localID} pairs under
        // non-guid keys (symbolID, guidPath, styleID). Only a node's own `guid`
        // identity may match — a same-valued symbolID must not. The decoy is
        // visited first, so an over-eager matcher would wrongly return it.
        let tree = json!({
            "children": [
                {
                    "name": "Decoy",
                    "guid": { "localID": 1, "sessionID": 6886 },
                    "symbolID": { "localID": 48774, "sessionID": 6886 }
                },
                {
                    "name": "Real",
                    "guid": { "localID": 48774, "sessionID": 6886 }
                }
            ]
        });

        let found = find_node(&tree, &id(6886, 48774)).unwrap();

        assert_eq!(found["name"], "Real");
    }

    #[test]
    fn find_node_matches_guids_that_survive_build_tree() {
        // Targeting runs on the build_tree output and before the passes that strip
        // guids, so a node's guid must still be present and matchable there. This
        // pins that ordering invariant against a refactor that builds differently.
        use crate::schema::build_tree;
        let tree = build_tree(vec![
            json!({ "guid": { "sessionID": 0, "localID": 0 }, "name": "Root" }),
            json!({
                "guid": { "sessionID": 6886, "localID": 48774 },
                "parentIndex": { "guid": { "sessionID": 0, "localID": 0 }, "position": "a" },
                "name": "Target"
            }),
        ])
        .unwrap();

        let found = find_node(&tree, &id(6886, 48774)).expect("guid should survive build_tree");
        assert_eq!(found["name"], "Target");
    }

    #[test]
    fn targeting_the_document_root_returns_the_whole_tree() {
        // build_tree roots the document at the synthetic guid 0:0, so targeting it
        // scopes to the entire document rather than a frame.
        use crate::schema::build_tree;
        let tree = build_tree(vec![
            json!({ "guid": { "sessionID": 0, "localID": 0 }, "name": "Root" }),
            json!({
                "guid": { "sessionID": 0, "localID": 1 },
                "parentIndex": { "guid": { "sessionID": 0, "localID": 0 }, "position": "a" },
                "name": "Child"
            }),
        ])
        .unwrap();

        let found = find_node(&tree, &id(0, 0)).unwrap();

        assert_eq!(found["name"], "Root");
        assert!(found.get("children").is_some());
    }

    #[test]
    fn rejects_out_of_range_id() {
        assert_eq!(parse_node_id("99999999999999999999:1"), None);
    }
}
