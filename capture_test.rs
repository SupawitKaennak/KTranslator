use screenshots::Screen;
use std::time::Instant;

fn main() {
    let screens = Screen::all().unwrap();
    let primary = screens.iter().find(|s| s.display_info.is_primary).unwrap();
    println!("Primary screen: {:?}", primary.display_info);
    
    let start = Instant::now();
    let image = primary.capture_area(0, 0, 100, 100).unwrap();
    println!("Capture took: {:?}", start.elapsed());
    println!("Image size: {}x{}", image.width(), image.height());
}
