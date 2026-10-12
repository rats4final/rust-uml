/// What kind of file a save is for (sets the dialog filter and extension).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    Model,
    Svg,
    Png,
}

impl FileKind {
    pub fn extension(self) -> &'static str {
        match self {
            FileKind::Model => "mdj",
            FileKind::Svg => "svg",
            FileKind::Png => "png",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            FileKind::Model => "StarUML model",
            FileKind::Svg => "SVG image",
            FileKind::Png => "PNG image",
        }
    }
}

#[derive(Debug)]
pub enum FileEvent {
    Opened {
        name: String,
        path: Option<String>,
        text: String,
    },
    Saved {
        kind: FileKind,
        name: String,
        path: Option<String>,
    },
    Failed(String),
}

/// File access, implemented per platform. Requests may complete later (in a
/// browser, dialogs are asynchronous); the UI collects results with [`poll`].
///
/// [`poll`]: FileService::poll
pub trait FileService {
    /// Ask the user for a model file to open.
    fn open_model(&mut self);

    /// Write `bytes`. With `path`, write there directly where the platform
    /// allows it; otherwise ask the user where (or download, in a browser).
    fn save(&mut self, kind: FileKind, suggested_name: &str, path: Option<&str>, bytes: Vec<u8>);

    /// Results of finished requests.
    fn poll(&mut self) -> Vec<FileEvent>;

    /// Whether the platform can read and write typed-in paths (desktop),
    /// as a fallback where no file dialog is available.
    fn has_paths(&self) -> bool {
        false
    }

    /// Open a model from a typed-in path. Only called if [`has_paths`].
    ///
    /// [`has_paths`]: FileService::has_paths
    fn open_path(&mut self, _path: &str) {}
}
