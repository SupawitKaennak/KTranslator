# KTranslator V2

[ภาษาไทย (Thai)](#thai) | [English](#english)

---
<img width="1917" height="1126" alt="{DD417730-D74E-48D7-B8F4-86A07DC7E0A6}" src="https://github.com/user-attachments/assets/b46b82c1-8395-4023-9a4f-83da7191a998" /><br>
<img width="1770" height="1068" alt="{C3BE31E9-110E-4894-8475-AFF12EEAE2B5}" src="https://github.com/user-attachments/assets/f1f0f849-1008-4acb-8806-6d7137ac9a33" /><br>
<img width="1601" height="641" alt="{9F263879-62FF-4DBC-881C-12D1DACB282B}" src="https://github.com/user-attachments/assets/cb1260b8-4180-4c3c-b8ed-d5b8c94d567f" /><br>
<img width="2554" height="1440" alt="{2A531C0F-AEC0-4498-9C81-D541733D3BD8}" src="https://github.com/user-attachments/assets/d278fe67-4039-4216-ad13-4a62af842980" /><br>

<a name="thai"></a>
## ภาษาไทย (Thai)

โปรแกรมแปลภาษาจากการจับภาพหน้าจอ (Screen Translator) รุ่นที่ 2 พัฒนาด้วยภาษา Rust เพื่อความรวดเร็วและประสิทธิภาพสูงสุด

### ลักษณะการใช้งาน
- **แปลเกม:** ใช้แปลบทสนทนาหรือเมนูในเกม รองรับการรันแบบ Overlay ทับหน้าต่างเกม
- **แปลมังงะ:** อ่านข้อความจากภาพมังงะหรือคอมมิค (รองรับตัวหนังสือแนวตั้ง/เอียง/โค้ง)
- **Professional Manga Mode:** ระบบ AI พิเศษ (Manga-OCR + YOLOv8) เพื่อการอ่านภาษาญี่ปุ่นแนวตั้งที่แม่นยำที่สุด
- **Auto Bubble Detection:** ระบบค้นหาลูกโป่งคำพูดอัตโนมัติ ลากกรอบคลุมทั้งหน้าแล้วปล่อยให้ AI จัดการ
- **แปลบทความ:** แปลข้อความจากหน้าเว็บ เอกสาร หรือ PDF ที่ไม่สามารถก๊อปปี้ข้อความได้

### ฟีเจอร์เด่น (Key Features)
- **Advanced Text Processing (NEW):** ระบบทำความสะอาดข้อความอัตโนมัติ เช่น ลบบรรทัดซ้ำ, ผสานบรรทัดที่ขาด, กรองอักษรขยะจาก OCR และสมานเศษประโยคซับไตเติล
- **Language-Specific Logic (NEW):** กระบวนการประมวลผลเฉพาะทาง เช่น ลบ Furigana (ญี่ปุ่น), แปลงอักษรจีนตัวย่อ/เต็ม และระบบตัดคำไทยอัจฉริยะ (Wordninja + Zero Width Space)
- **Regex Replacement Engine (NEW):** ระบบ Pipeline สำหรับจัดการข้อความด้วย Regular Expression ปรับแต่งการแทนที่หรือกรองคำได้อย่างอิสระ
- **Custom Glossary & Glossary (NEW):** ระบบพจนานุกรมและหน่วยความจำส่วนตัว (Translation Memory) สำหรับคุมคำศัพท์เฉพาะ เช่น ชื่อตัวละคร หรือชื่อสกิล
- **Image Pre-Processing (NEW):** ชุดเครื่องมือปรับแต่งภาพก่อนส่งให้ OCR เช่น Binarize, Adaptive Threshold, Contrast, Sharpen และระบบแก้ภาพเอียง (Deskew)
- **Translation Behavior Control:** ปรับแต่งพฤติกรรม AI (LLM) ได้อย่างละเอียด ทั้งการคุมโทนเสียง (Tone), ระดับภาษา และระบบ Realtime Stability ป้องกันข้อความกะพริบ
- **Customizable Overlay:** ปรับแต่งหน้าตาของหน้าต่างแปลได้ทุกส่วน ทั้งสีพื้นหลัง, ความโปร่งใส, ขนาดฟอนต์, และความโค้งมนของขอบ

### ความต้องการของระบบ (Requirements)

**1. ระบบ OCR (ตัวอ่านข้อความ)**
- **Manga-OCR (Recommended):** ระบบ AI (ONNX) รันผ่าน GPU แม่นยำที่สุดสำหรับมังงะ (ติดตั้งได้ทันทีผ่าน **Model Installation Center** ในตัวโปรแกรม)
- **Windows OCR:** (ติดมากับ Windows) รวดเร็วและใช้ทรัพยากรน้อย เหมาะสำหรับเอกสารทั่วไป (ต้องติดตั้ง Language Pack ใน Windows Settings ให้เรียบร้อย)
- **PaddleOCR:** เอนจินยอดนิยมสำหรับฟอนต์พิเศษ ต้องดาวน์โหลด [PaddleOCR-json](https://github.com/hiroi-sora/PaddleOCR-json/releases) และระบุที่อยู่ไฟล์ใน Settings

**2. ระบบการแปล (Translator)**
- **Google Translate (FREE):** แปลภาษาได้ทันทีโดยไม่ต้องใช้ API Key
- **AI Providers (LLM):** รองรับ **Gemini**, **Groq**, **Ollama (Offline)**, และ **Custom OpenAI** (เช่น OpenRouter, DeepSeek, LM Studio) พร้อมระบบ Auto-Fetch ดึงรายชื่อโมเดลอัตโนมัติ

### แหล่งที่มาของโมเดล (Model Resources)
- **Manga-OCR 2025 (ONNX):** [l0wgear/manga-ocr-2025-onnx](https://huggingface.co/l0wgear/manga-ocr-2025-onnx)
- **YOLOv8 Text Detection:** [deepghs/manga109_yolo](https://huggingface.co/deepghs/manga109_yolo) (v2023.12.07_s)

### เทคโนโลยีที่ใช้ (Tech Stack)
- **Language:** Rust (edition 2024)
- **UI Framework:** [egui](https://github.com/emilk/egui) (eframe)
- **Runtime:** ONNX Runtime (ort) พร้อมการเร่งความเร็วด้วย GPU (DirectML)
- **Capture:** Win32 API & dxgcap สำหรับการจับภาพความเร็วสูงและความลัดเชียบ (Transparent Overlay)

### การติดตั้งและใช้งาน

**วิธีติดตั้ง (สำหรับนักพัฒนา):**
1. ติดตั้ง [Rust Toolchain](https://rustup.rs/)
2. Clone โปรเจกต์:
   ```bash
   git clone https://github.com/SupawitKaennak/KTranslatorV2.git
   cd KTranslatorV2
   ```
3. รันโปรแกรม:
   ```bash
   cargo run --release
   ```
4. เมื่อโปรแกรมเปิดขึ้น ให้ไปที่ **Settings > OCR Engine** แล้วกด **Download & Install Models** เพื่อติดตั้ง AI Model อัตโนมัติ

**ขั้นตอนการใช้งาน:**
1. เข้าไปที่ **Settings** (ไอคอนฟันเฟือง) เพื่อเลือก OCR และใส่ API Key ของผู้ให้บริการที่ต้องการ
2. กด **Add Region** และเลือกพื้นที่บนหน้าจอที่ต้องการแปล
3. เลือกภาษาต้นทาง (From) และภาษาปลายทาง (To)
4. กดปุ่ม **Start** เพื่อเริ่มการแปล
5. เปิดโหมด **Overlay Mode** เพื่อให้คำแปลแสดงทับตำแหน่งเดิมบนหน้าจออย่างสวยงาม

---

<a name="english"></a>
## English

A powerful, high-performance Screen Translator built with Rust for real-time translation and seamless overlay experience.

### Use Cases
- **Game Translation:** Real-time dialogue and menu translation with low-latency overlay.
- **Manga/Comics:** Specialized OCR for vertical, stylized, and curved Japanese text.
- **Pro Manga Mode:** Integrated **Manga-OCR + YOLOv8** for state-of-the-art vertical Japanese recognition.
- **Auto Bubble Detection:** AI-driven speech bubble detection—just select the page and let the AI find the text.
- **Documents/Web:** Translate text from non-selectable sources like PDFs, protected websites, or images.

### Key Features
- **Advanced Text Processing (NEW):** Automatic cleanup including duplicate removal, broken line merging, OCR garbage filtering, and subtitle fragment reconstruction.
- **Language-Specific Logic (NEW):** Specialized processing for various languages: Furigana stripping (JP), Simp/Trad conversion (CN), Smart word segmentation (TH), and RTL correction (AR).
- **Regex Replacement Engine (NEW):** A powerful pipeline to transform or scrub text using custom Regular Expression rules.
- **Custom Glossary & Glossary (NEW):** Personal dictionary and Translation Memory to enforce terminology for character names, skills, and items.
- **Image Pre-Processing (NEW):** Robust image enhancement tools: Binarization, Adaptive Threshold, Contrast, Sharpening, and Auto-Deskew.
- **Translation Behavior Control:** Fine-tune AI (LLM) behavior with custom prompts, tone settings, and Realtime Stability logic to prevent flickering.
- **Customizable Overlay:** Full control over UI aesthetics including background/text colors, opacity, font size, and corner radius.

### System Requirements

**1. OCR Engines (Text Recognition)**
- **Manga-OCR (Recommended):** High-precision AI recognition (ONNX) with GPU support. Installable with one click via the **Model Installation Center**.
- **Windows OCR:** Built-in and extremely fast. Best for standard documents. (Requires Windows Language Packs).
- **PaddleOCR:** Versatile engine for stylized fonts. Download [PaddleOCR-json](https://github.com/hiroi-sora/PaddleOCR-json/releases) and link the path in settings.

**2. Translation Providers**
- **Google Translate (FREE):** Instant translation without an API Key.
- **AI Providers (LLM):** Supports **Gemini**, **Groq**, **Ollama (Offline)**, and **Custom OpenAI** (OpenRouter, DeepSeek, etc.) with **Auto-Fetch** model selection.

### Tech Stack
- **Language:** Rust (edition 2024)
- **UI Framework:** [egui](https://github.com/emilk/egui) (eframe)
- **AI Runtime:** ONNX Runtime (ort) with DirectML (GPU Acceleration)
- **Capture & Overlay:** Win32 API & dxgcap for high-speed, transparent frame capture.

### Getting Started

**Installation (Developers):**
1. Install [Rust Toolchain](https://rustup.rs/).
2. Clone the repository:
   ```bash
   git clone https://github.com/SupawitKaennak/KTranslatorV2.git
   cd KTranslatorV2
   ```
3. Run the application:
   ```bash
   cargo run --release
   ```
4. Once open, go to **Settings > OCR Engine** and click **Download & Install Models** to set up the AI models automatically.

**Basic Usage:**
1. Open **Settings** (gear icon) to select your OCR engine and enter API keys.
2. Click **Add Region** to select the screen area you want to translate.
3. Select Source (From) and Target (To) languages.
4. Click **Start** to begin the real-time translation loop.
5. Enable **Overlay Mode** to display translations directly over the original text.

### AI Models & Credits
- **Manga-OCR 2025 (ONNX):** [l0wgear/manga-ocr-2025-onnx](https://huggingface.co/l0wgear/manga-ocr-2025-onnx)
- **YOLOv8 Text Detection:** [deepghs/manga109_yolo](https://huggingface.co/deepghs/manga109_yolo) (v2023.12.07_s)

---

### License
Copyright (c) 2026 Supawit Kaennak [GPL v3.0](LICENSE). All rights reserved.
