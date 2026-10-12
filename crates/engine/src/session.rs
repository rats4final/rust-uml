use std::collections::HashSet;

use mdj::{Document, Editor, Element, Id, LoadError};
use render::{Fonts, Scene};

use crate::CommandError;

/// One open project and what the user is looking at.
pub struct Session {
    pub doc: Document,
    /// Display name, e.g. `Untitled.mdj`.
    pub file_name: String,
    /// Where the file lives, when the platform has paths (not in browsers).
    pub path: Option<String>,
    pub active_diagram: Option<Id>,
    /// Selected views and/or model elements.
    pub selection: Vec<Id>,
    fonts: Fonts,
}

/// What an edit touched, so geometry can be brought up to date afterwards.
#[derive(Default)]
pub(crate) struct Changes {
    /// Node views whose attached edges must be re-routed.
    pub touched: HashSet<Id>,
    /// Edges the command already positioned itself.
    pub keep: HashSet<Id>,
    /// Diagrams to re-check besides the active one.
    pub diagrams: HashSet<Id>,
}

impl Changes {
    /// Note that `model` changed, so every diagram showing it is re-checked
    /// (a longer name can make its boxes grow).
    pub fn model_changed(&mut self, doc: &Document, model: &str) {
        for view in uml::views_of(doc, model) {
            if let Some(d) = diagram_of(doc, &view) {
                self.diagrams.insert(d);
            }
        }
    }
}

/// The diagram a view belongs to.
pub fn diagram_of(doc: &Document, view: &str) -> Option<Id> {
    let mut cur = doc.get(view)?;
    // Bounded by tree depth; parents cannot form a cycle in a loaded tree,
    // but cap it anyway.
    for _ in 0..256 {
        if is_diagram(cur) {
            return Some(cur.id.clone());
        }
        cur = doc.get(cur.parent.as_deref()?)?;
    }
    None
}

/// Diagram types all end in `Diagram` (`UMLClassDiagram`, `BPMNDiagram`, …);
/// no view type does.
pub fn is_diagram(el: &Element) -> bool {
    el.type_name.ends_with("Diagram")
}

impl Default for Session {
    fn default() -> Self {
        Self::from_document(Document::new_project(), "Untitled.mdj".into(), None)
    }
}

impl Session {
    pub fn from_document(doc: Document, file_name: String, path: Option<String>) -> Self {
        let active_diagram = default_diagram(&doc);
        Self {
            doc,
            file_name,
            path,
            active_diagram,
            selection: Vec::new(),
            fonts: Fonts::default(),
        }
    }

    /// Open a project from file contents.
    pub fn open(text: &str, file_name: String, path: Option<String>) -> Result<Self, LoadError> {
        Ok(Self::from_document(
            Document::from_json_str(text)?,
            file_name,
            path,
        ))
    }

    pub fn fonts(&self) -> &Fonts {
        &self.fonts
    }

    /// The display list of the active diagram.
    pub fn scene(&self) -> Option<Scene> {
        let d = self.active_diagram.as_deref()?;
        Some(render::build_scene(&self.doc, d, &self.fonts))
    }

    /// Serialise for saving. Call [`Session::mark_saved`] once the bytes
    /// have actually been written.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        self.doc.to_json_string()
    }

    pub fn mark_saved(&mut self) {
        self.doc.mark_saved();
    }

    /// The model element behind the first selected item (a view's model, or
    /// the element itself).
    pub fn selected_model(&self) -> Option<&Element> {
        let first = self.doc.get(self.selection.first()?)?;
        match first.ref_id("model") {
            Some(m) if first.get("left").is_some() || first.get("head").is_some() => {
                self.doc.get(m)
            }
            _ => Some(first),
        }
    }

    /// Run an edit as one undo step, then bring geometry up to date in the
    /// active diagram and any diagram the edit noted. On error nothing changes.
    pub(crate) fn edit<R>(
        &mut self,
        f: impl FnOnce(&mut Editor, &mut Changes) -> Result<R, CommandError>,
    ) -> Result<R, CommandError> {
        let fonts = &self.fonts;
        let active = self.active_diagram.clone();
        self.doc.try_edit(|e| {
            let mut changes = Changes::default();
            let result = f(e, &mut changes)?;
            let mut diagrams: Vec<Id> = changes.diagrams.iter().cloned().collect();
            diagrams.sort();
            if let Some(a) = active {
                diagrams.retain(|d| *d != a);
                diagrams.insert(0, a);
            }
            for d in diagrams {
                render::reflow(e, &d, &changes.touched, &changes.keep, fonts);
            }
            Ok(result)
        })
    }

    /// Drop selected ids that no longer exist and fall back to another
    /// diagram if the active one was deleted.
    pub(crate) fn prune(&mut self) {
        let doc = &self.doc;
        self.selection.retain(|id| doc.get(id).is_some());
        if self
            .active_diagram
            .as_deref()
            .is_none_or(|d| doc.get(d).is_none())
        {
            self.active_diagram = default_diagram(doc);
        }
    }
}

/// The diagram flagged `defaultDiagram`, else the first diagram in tree order.
pub fn default_diagram(doc: &Document) -> Option<Id> {
    let all = doc.subtree_ids(doc.root());
    let diagrams: Vec<&Element> = all
        .iter()
        .filter_map(|id| doc.get(id))
        .filter(|e| is_diagram(e))
        .collect();
    diagrams
        .iter()
        .find(|d| d.bool("defaultDiagram", false))
        .or(diagrams.first())
        .map(|d| d.id.clone())
}
