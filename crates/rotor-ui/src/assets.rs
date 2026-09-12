use gpui_kit::{AssetSource, Result, SharedString, assets::Assets};
use std::borrow::Cow;

gpui_kit::assets::icon_assets!(
    ExtraIcons,
    [
        CirclePlus,
        Pencil,
        Trash,
        Square,
        MoveUpRight,
        Type,
        ScanText,
        Download
    ]
);

/// Default component assets plus the additional icons used by Rotor's controls.
pub struct UiAssets;

impl AssetSource for UiAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        let search_icon: Option<&'static [u8]> = match path {
            "search/ai.svg" => Some(include_bytes!("search_icons/ai.svg")),
            "search/search.svg" => Some(include_bytes!("search_icons/search.svg")),
            "search/admin.svg" => Some(include_bytes!("search_icons/admin.svg")),
            "search/folder.svg" => Some(include_bytes!("search_icons/folder.svg")),
            _ => None,
        };
        if let Some(bytes) = search_icon {
            return Ok(Some(Cow::Borrowed(bytes)));
        }
        match ExtraIcons.load(path)? {
            Some(bytes) => Ok(Some(bytes)),
            None => Assets.load(path),
        }
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut paths = Assets.list(path)?;
        paths.extend(ExtraIcons.list(path)?);
        paths.extend(
            [
                "search/search.svg",
                "search/ai.svg",
                "search/admin.svg",
                "search/folder.svg",
            ]
            .into_iter()
            .filter(|icon| icon.starts_with(path))
            .map(SharedString::from),
        );
        paths.sort();
        paths.dedup();
        Ok(paths)
    }
}
