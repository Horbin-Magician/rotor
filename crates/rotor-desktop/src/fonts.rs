use gpui_kit::*;
use rotor_common::ResourceLocator;
use std::{borrow::Cow, path::Path};

pub fn load(resources: Option<ResourceLocator>, cx: &mut App) -> Task<()> {
    let text_system = cx.text_system().clone();
    cx.spawn(async move |cx| {
        let result = cx
            .background_executor()
            .spawn(async move {
                let resource = resources.ok_or("Annotation resources are unavailable")?;
                let path = resource
                    .resolve(Path::new("fonts/NotoSansCJKsc-Regular.otf"))
                    .map_err(|error| error.to_string())?;
                let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
                text_system
                    .add_fonts(vec![Cow::Owned(bytes)])
                    .map_err(|error| error.to_string())
            })
            .await;
        cx.update(|cx| {
            if let Err(error) = result {
                eprintln!("Annotation font: {error}");
                crate::publish_warning(error, cx);
            }
            cx.refresh_windows();
        });
    })
}
