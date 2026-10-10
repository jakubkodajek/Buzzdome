//! Categories (spec QZ-10): named looks a quiz keeps, each a background
//! and a style over the theme. Applying a category copies its look onto
//! slides (the editor's `formatLike`), which remember it by id, so it can be
//! applied to them again and a spreadsheet row can name it (`CAT`).

use crate::media::{check_reference, Background, BACKGROUND_KINDS};
use crate::model::Problem;
use crate::style::SlideStyle;
use crate::text::name_key;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use ts_rs::TS;

/// Longest category name, in characters.
pub const MAX_CATEGORY_NAME_CHARS: usize = 40;
/// Longest category id, in characters; the editor makes ones like `c-k3x9q2ab`.
pub const MAX_CATEGORY_ID_CHARS: usize = 64;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct Category {
    pub id: String,
    /// Unique in the quiz regardless of case, as a spreadsheet's `CAT` names it.
    pub name: String,
    /// Picture or video behind the slides.
    #[serde(default, with = "crate::media::background_any_version")]
    #[ts(as = "Option<Background>")]
    pub background: Option<Background>,
    /// Colours, gradients and fonts over the theme's.
    #[serde(default, skip_serializing_if = "SlideStyle::is_empty", with = "crate::style::lenient_or_default")]
    #[ts(as = "SlideStyle")]
    pub style: SlideStyle,
}

impl Category {
    /// A category of the theme's look.
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Category {
        Category { id: id.into(), name: name.into(), background: None, style: SlideStyle::default() }
    }

    /// The media file this category uses.
    pub fn media(&self) -> Option<&str> {
        self.background.as_ref().map(|background| background.file.as_str())
    }

    /// The same media reference as `media`, to change in place.
    pub(crate) fn media_mut(&mut self) -> Option<&mut String> {
        self.background.as_mut().map(|background| &mut background.file)
    }
}

/// The category of `categories` with this name (see `name_key`).
pub fn category_named<'a>(categories: &'a [Category], name: &str) -> Option<&'a Category> {
    let key = name_key(name);
    categories.iter().find(|category| name_key(&category.name) == key)
}

/// The first of "`base` 1", "`base` 2"… that no category of `categories` is
/// named (see `name_key`), for a new category.
pub fn fresh_category_name(categories: &[Category], base: &str) -> String {
    let taken: HashSet<String> = categories.iter().map(|category| name_key(&category.name)).collect();
    // Of one number more than there are categories, one is free.
    (1..=categories.len() + 1)
        .map(|n| format!("{} {n}", base.trim()))
        .find(|name| !taken.contains(&name_key(name)))
        .expect("one of the numbers is free")
}

/// What is wrong with the quiz's categories, each with the category's name.
pub(crate) fn category_problems(categories: &[Category], found: &mut impl FnMut(Problem)) {
    let mut ids = HashSet::new();
    let mut names = HashSet::new();
    for category in categories {
        let name = category.name.trim();
        let id = category.id.as_str();
        if id.is_empty() || id.chars().count() > MAX_CATEGORY_ID_CHARS {
            found(Problem::CategoryIdInvalid { name: name.to_string(), max: MAX_CATEGORY_ID_CHARS });
        } else if !ids.insert(id) {
            found(Problem::DuplicateCategoryId { name: name.to_string() });
        }
        if name.is_empty() {
            found(Problem::CategoryUnnamed);
        } else if name.chars().count() > MAX_CATEGORY_NAME_CHARS {
            found(Problem::CategoryNameTooLong { max: MAX_CATEGORY_NAME_CHARS });
        } else if !names.insert(name_key(name)) {
            found(Problem::CategoryNameTaken { name: name.to_string() });
        }
        if let Some(file) = category.media() {
            if let Err(issue) = check_reference(file, BACKGROUND_KINDS) {
                found(Problem::CategoryMedia { name: name.to_string(), file: file.to_string(), issue });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::MediaIssue;

    fn problems(categories: &[Category]) -> Vec<Problem> {
        let mut found = Vec::new();
        category_problems(categories, &mut |problem| found.push(problem));
        found
    }

    #[test]
    fn names_are_unique_regardless_of_case_and_spaces_and_found_so() {
        let categories = [Category::new("c1", "Science"), Category::new("c2", " science ")];
        assert_eq!(problems(&categories), [Problem::CategoryNameTaken { name: "science".into() }]);
        assert_eq!(category_named(&categories[..1], "  SCIENCE").map(|c| c.id.as_str()), Some("c1"));
        assert_eq!(category_named(&categories[..1], "Music"), None);
    }

    #[test]
    fn a_new_category_gets_the_first_numbered_name_none_has_regardless_of_case_and_spaces() {
        let categories = [Category::new("c1", " category 1 "), Category::new("c2", "CATEGORY 3")];
        assert_eq!(fresh_category_name(&categories, "Category"), "Category 2");
        assert_eq!(fresh_category_name(&categories[..1], " Category "), "Category 2");
        assert_eq!(fresh_category_name(&[], "Category"), "Category 1");
    }

    #[test]
    fn a_category_needs_its_own_id_of_a_length_a_name_of_a_length_and_a_safe_background() {
        let mut long = Category::new("c3", "x".repeat(MAX_CATEGORY_NAME_CHARS + 1));
        long.background = Some(Background::new("../outside.mp4"));
        let found = problems(&[Category::new("c1", "A"), Category::new("c1", " "), long]);
        assert_eq!(found[0], Problem::DuplicateCategoryId { name: String::new() });
        assert_eq!(found[1], Problem::CategoryUnnamed);
        assert_eq!(found[2], Problem::CategoryNameTooLong { max: MAX_CATEGORY_NAME_CHARS });
        assert!(matches!(&found[3], Problem::CategoryMedia { file, issue: MediaIssue::NotInMediaFolder, .. } if file == "../outside.mp4"), "{found:?}");
        assert!(problems(&[Category::new("c1", "x".repeat(MAX_CATEGORY_NAME_CHARS))]).is_empty());
        let invalid = |name: &str| Problem::CategoryIdInvalid { name: name.into(), max: MAX_CATEGORY_ID_CHARS };
        let ids = [Category::new("", "B"), Category::new("x".repeat(MAX_CATEGORY_ID_CHARS + 1), "C"), Category::new("x".repeat(MAX_CATEGORY_ID_CHARS), "D")];
        assert_eq!(problems(&ids), [invalid("B"), invalid("C")]);
    }
}
