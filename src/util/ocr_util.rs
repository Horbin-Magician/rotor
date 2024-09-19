use image::DynamicImage;
use windows::Media::Ocr::{self};
use windows::Graphics::Imaging::{BitmapPixelFormat, SoftwareBitmap};
use windows::Globalization::Language;
use windows::Storage::Streams::DataWriter;

// Contains the text and the x,y coordinates of the word
pub struct Coordinates {
    pub text:           String,
    pub x :             f32,
    pub y :             f32,
    pub height:         f32,
    pub width:          f32
}

pub fn ocr_with_bounds(image: DynamicImage) -> windows::core::Result<Vec<Coordinates>> {
    // Convert the DynamicImage to RGBA8 format
    let rgba_image = image.to_rgba8();
    let (width, height) = rgba_image.dimensions();
    let pixels = rgba_image.into_raw();

    // Create a DataWriter to write the pixel data
    let data_writer = DataWriter::new()?;
    data_writer.WriteBytes(&pixels)?;

    // Create a SoftwareBitmap from the pixel data
    let software_bitmap = SoftwareBitmap::CreateCopyFromBuffer(
        &data_writer.DetachBuffer()?,
        BitmapPixelFormat::Bgra8,
        width as i32,
        height as i32,
    )?;

    let langs = Ocr::OcrEngine::AvailableRecognizerLanguages()?;
    let lang = &langs.GetAt(1)?.LanguageTag()?;

    for l in langs.into_iter() {
        println!("{:?}", l.LanguageTag());
    }

    let lang = Language::CreateLanguage(lang)?;
    let engine = Ocr::OcrEngine::TryCreateFromLanguage(&lang)?;

    let result = engine
        .RecognizeAsync(&software_bitmap)?
        .get()?
        .Lines()?
        ;
    
    let mut collected_words:Vec<Coordinates> = Vec::new();    
    
    result.into_iter().for_each(|line|{
        let line_text = line.Text().unwrap().to_string_lossy();
        let words = line.Words().unwrap();

        let mut pos_x: f32 = 0.0;
        let mut pos_y: f32 = 0.0;
        let mut line_heigth: f32 = 0.0;
        let mut line_width: f32 = 0.0;

        let mut idx = 0;
        words.into_iter().for_each(|word|{ // TODO if right
            let rect = word.BoundingRect().unwrap();

            if idx == 0 { pos_x = rect.X; }

            if line_heigth < rect.Height {
                line_heigth = rect.Height;
            }

            line_width += rect.Width; // TODO check why
            if pos_y < rect.Y {
                pos_y = rect.Y;
            }
            idx +=1;
        });

        collected_words.push(
            Coordinates{
                x: pos_x, 
                y: pos_y, 
                text: line_text,
                height: line_heigth,
                width: line_width
            }
        )
    });

    Ok(collected_words)
}