# KTranslator V2
<img width="2290" height="1314" alt="{D0FD1C9F-76EE-40F6-ACDC-8BA0DBB8BCF9}" src="https://github.com/user-attachments/assets/55f7f3f6-4f9e-4e13-9b1d-b9f1ea110aba" /><br>
<img width="2364" height="1265" alt="{5A9B19F6-D00B-4864-B056-41FC0FED722C}" src="https://github.com/user-attachments/assets/30cf32da-44ad-4686-9852-3028a4b82775" /><br>
<img width="2562" height="1440" alt="{6E9BAEBF-E11A-4561-8FF7-583830210EDA}" src="https://github.com/user-attachments/assets/cee5db85-4186-4c21-8608-c5c676a4ff01" /><br>

[ภาษาไทย (Thai)](#thai) | [English](#english)

---

<a name="thai"></a>
## ภาษาไทย (Thai)

โปรแกรมแปลภาษาจากการจับภาพหน้าจอ (Screen Translator) ที่เขียนด้วยภาษา Rust โดยใช้เฟรมเวิร์ก egui สำหรับหน้าจอส่วนติดต่อผู้ใช้ และใช้ ONNX Runtime ในการประมวลผลโมเดลรู้จำอักขระ (OCR) 

### สิ่งที่โปรแกรมทำได้
- **การเลือกพื้นที่เพื่อจับภาพหน้าจอ**: รองรับการกำหนดขอบเขตพื้นที่จับภาพ (Capture Region) บนหน้าจอเพื่อแปลข้อความในจุดที่เลือก สามารถย้ายตำแหน่งหรือขยายขนาดกล่องเลือกได้ผ่าน UI และมีโหมดแสดงคำแปลทับตำแหน่งข้อความเดิม (Overlay Mode)
- **ตัวเลือกเอนจิน OCR (ONNX Runtime)**:
  - Manga-OCR: รันโมเดลปัญญาประดิษฐ์สแกนตัวอักษรภาษาญี่ปุ่นทั้งในแนวตั้งและแนวนอน เหมาะสำหรับการแปลหนังสือการ์ตูนมังงะ
  - PaddleOCR v4 (Mobile): โมเดลสแกนตัวอักษรรุ่นขนาดเล็กเพื่อลดการใช้แรมและทรัพยากรคอมพิวเตอร์
  - PaddleOCR v4 (Server): โมเดลรุ่นมาตรฐานเพื่อความละเอียดและความถูกต้องบนฟอนต์หรือหน้าเอกสารที่ซับซ้อน
  - Windows OCR: เรียกใช้งานระบบรู้จำอักขระผ่าน API บิวท์อินของระบบปฏิบัติการ Windows ทันทีโดยไม่ต้องทำการดาวน์โหลดไฟล์โมเดลเพิ่มเติม
- **การเร่งความเร็วประมวลผลด้วย GPU (Hardware Acceleration)**: รองรับการเชื่อมต่อกับไดรเวอร์การ์ดจอ NVIDIA CUDA, TensorRT และ DirectML บนระบบปฏิบัติการ Windows ช่วยสลับการรันโมเดล ONNX ไปยังหน่วยประมวลผลกราฟิก
- **การวิเคราะห์การจัดหน้า (Layout Analysis)**:
  - การตรวจจับกรอบคำพูด (Speech Bubble Detection): ค้นหากรอบคำพูดของมังงะด้วยโมเดลตรวจจับวัตถุ YOLO
  - การเรียงลำดับคำสแกน (Smart Sorting): จัดทิศทางของข้อความให้อ่านจากขวาไปซ้าย (RTL) หรือจากบนลงล่างให้สอดคล้องกับโครงสร้างหน้าหนังสือมังงะ
- **การประมวลผลข้อความ (Text Processing)**:
  - ระบบคัดกรองขยะ (Garbage Filtering): ตรวจจับและลบตัวอักษรซ้ำ บรรทัดเปล่า หรือสัญลักษณ์พิเศษที่ไม่ต้องการออกก่อนนำไปแปล
  - การแปลงภาษาญี่ปุ่น: ปรับรูปแบบอักขระคานะ ลบคำถอดเสียงอ่านฟูริกานะ (Furigana) และตั้งค่าให้คงเหลือคำลงท้ายแสดงความเคารพ (Honorifics)
  - การตัดบรรทัดภาษาไทย: เพิ่มช่องว่างขนาดศูนย์ (Zero Width Space) เข้าในข้อความเพื่อช่วยให้ภาษาไทยแสดงผลล้นขอบหรือตัดบรรทัดได้เหมาะสม
  - การแยกคำภาษาอังกฤษ: เชื่อมต่ออัลกอริทึม wordninja เพื่อแยกตัวอักษรที่ติดกันเป็นคำๆ ตามพจนานุกรมความถี่คำภาษาอังกฤษ
- **ระบบแปลภาษา (Translation Providers)**:
  - Google Translate: แปลภาษาผ่านเว็บฟรี ไม่จำเป็นต้องลงทะเบียนคีย์ใช้งาน
  - Gemini API: เชื่อมต่อผ่านรหัสคีย์ที่ได้รับจาก Google AI Studio
  - Groq API: ส่งคำแปลไปยังบริการของ Groq ผ่านโมเดลเช่น Llama หรือ Gemma
  - Ollama: รองรับการติดต่อเซิร์ฟเวอร์ Ollama ภายในเครื่องคอมพิวเตอร์เพื่อแปลภาษาแบบออฟไลน์ 100%
  - OpenAI / Custom: ส่งคำขอแปลด้วยโครงสร้างแบบ OpenAI API ไปยังเซิร์ฟเวอร์ส่วนตัว หรือผู้ให้บริการอื่นๆ เช่น OpenRouter และ DeepSeek
- **การจัดการความจำชั่วคราวและการตกแต่งหน้าตา**:
  - ระบบแคชข้อความ (Translation Cache): ทำการเก็บประวัติข้อความและผลลัพธ์แปลในหน่วยความจำระดับเฟรมและประโยค เพื่อลดการดึงข้อมูลแปลจาก API ซ้ำซ้อนเมื่อหน้าจอไม่เคลื่อนไหว
  - การเปลี่ยนรูปแบบตัวอักษร: ปรับสีตัวอักษร ขนาดฟอนต์ สีพื้นหลัง และระดับความโปร่งแสงของพื้นหลังคำแปลในโหมด Overlay

### การเตรียมความพร้อมก่อนใช้งาน
1. **การดาวน์โหลดและติดตั้งโมเดล**:
   - เปิดโปรแกรมแล้วคลิกไปที่เมนู Settings (ไอคอนฟันเฟือง) จากนั้นเลือกแท็บ OCR
   - ในส่วนการตั้งค่าของ MangaOCR หรือ Built-in PaddleOCR ระบบจะตรวจหาไฟล์โมเดลในเครื่อง หากไม่พบจะแสดงปุ่ม Download และคำแจ้งเตือนโมเดลสูญหาย
   - ให้คลิกปุ่ม Download เพื่อให้ระบบดำเนินการดาวน์โหลดไฟล์และจัดวางลงในโฟลเดอร์ models/manga-ocr/ หรือ models/ppocr/ ในไดเรกทอรีทำงานโดยอัตโนมัติ
2. **การติดตั้งโปรแกรมสนับสนุนการประมวลผล GPU**:
   - ผู้ใช้การ์ดจอ NVIDIA ที่ต้องการเร่งความเร็วการประมวลผล ให้ติดตั้งไดรเวอร์กราฟิกล่าสุด ติดตั้ง [CUDA Toolkit](https://developer.nvidia.com/cuda-downloads) และตั้งค่าความเข้ากันได้ของ TensorRT ในส่วนของการตั้งค่าเครื่องรันไทม์ ONNX
   - อุปกรณ์ทั่วไปสามารถประมวลผลผ่าน CPU หรือเรียกใช้ DirectML ของ Microsoft Windows ได้ทันที
3. **การสมัครคีย์เชื่อมต่อบริการแปลภาษา (API Setup)**:
   - นำคีย์หรือที่อยู่เซิร์ฟเวอร์ที่สมัครมาใส่ในหน้า Settings > Translation:
     - Google Translate: พร้อมใช้งานทันที
     - Gemini: ลงทะเบียนขอคีย์ที่ [Google AI Studio](https://aistudio.google.com/)
     - Groq: สมัครสมาชิกและออกคีย์ที่ [Groq Console](https://console.groq.com/)
     - Ollama: ดาวน์โหลดโปรแกรมและสั่งรันโมเดลโลคอลจากเว็บ [Ollama.com](https://ollama.com/)
     - OpenAI / Custom: ขอสิทธิ์การใช้งานจาก [OpenAI Platform](https://platform.openai.com/) หรือ [OpenRouter](https://openrouter.ai/)

### วิธีใช้งาน
1. เรียกใช้โปรแกรมโดยเปิดไฟล์ ktranslator.exe ในโฟลเดอร์โครงการ
2. คลิกปุ่มตั้งค่ารูปฟันเฟืองบริเวณหน้าต่างหลักเพื่อกำหนดค่าเริ่มต้น
3. เลือกแท็บ OCR เพื่อกำหนดภาษาเริ่มต้น สไตล์เนื้อหา (เช่น เกม มังงะ หรือเอกสาร) และเลือกเอนจินสแกนข้อความ
4. เลือกแท็บ Translation เพื่อระบุบริการแปลภาษา คีย์ใช้งาน และภาษาต้นทางกับปลายทาง
5. กลับมาที่หน้าต่างหลัก กดปุ่ม Add Region แล้วคลิกเมาส์ลากครอบพื้นที่หน้าต่างภาพหรือเกมที่ต้องการระบบแปล
6. กดปุ่ม Start Translate เพื่อเปิดการตรวจจับและแปลงคำศัพท์
7. หากต้องการให้ตัวหนังสือแสดงทับข้อความเดิม ให้กดเปิดใช้งาน Overlay Mode จากแผงควบคุมหน้าจอ และปรับแต่งสีและขนาดฟอนต์ตามต้องการ

---

<a name="english"></a>
## English

KTranslator V2 is a screen capture translation utility written in Rust. It utilizes the egui framework for its graphical interface and ONNX Runtime to execute artificial intelligence optical character recognition (OCR).

### Key Features
- **Region-Based Capture**: Allows users to select specific bounding boxes on the screen for target scanning. Bounding boxes can be scaled or moved via the user interface. Translations can be overlayed directly on top of the original text using Overlay Mode.
- **OCR Engine Selection (ONNX Runtime)**:
  - Manga-OCR: Runs a neural network model to read both vertical and horizontal Japanese writing, optimized for manga books.
  - PaddleOCR v4 (Mobile): A lightweight model variant designed to reduce system memory and computing footprint.
  - PaddleOCR v4 (Server): A standard model variant providing higher accuracy for complex text structures and fonts.
  - Windows OCR: Integrates with the built-in OCR library of the Windows operating system, running instantly without downloading additional model files.
- **Hardware Acceleration**: Connects with NVIDIA CUDA, TensorRT, and DirectML APIs on Windows, allowing character recognition models to run on dedicated graphics processing units.
- **Layout Analysis**:
  - Speech Bubble Detection: Identifies speech bubbles inside manga layout boundaries using YOLO object detection.
  - Smart Sorting: Orders text boxes in a column-major reading sequence (such as right-to-left for Japanese layouts).
- **Text Processing**:
  - Garbage Filtering: Identifies and removes repeated patterns, blank rows, and irregular symbol combinations before translation.
  - Japanese Normalization: Formats kana text, removes furigana phonetic markings, and provides options to preserve or drop honorific markers.
  - Thai Word Wrap: Inserts Zero Width Spaces into Thai translations to enable clean line breaks and prevent boundary overflow.
  - English Word Segmentation: Utilizes the wordninja algorithm to separate merged English character runs into standard terms.
- **Supported Translation Providers**:
  - Google Translate: Free online translation provider that operates without requiring credential keys.
  - Gemini API: Passes text to Google Gemini models using API keys obtained from Google AI Studio.
  - Groq API: Sends requests to the Groq inference engine to run translation prompts via models like Llama or Gemma.
  - Ollama: Targets a local Ollama server on your machine for 100% offline large language model translations.
  - OpenAI / Custom: Sends OpenAI-compliant translation API requests to custom server endpoints, including OpenRouter or DeepSeek.
- **Performance and Rendering Settings**:
  - Caching Engine: Saves frame hashes and text segments inside local memory to prevent redundant API queries when static images are displayed.
  - Custom Styles: Modifies font colors, text dimensions, background fills, and overlay alpha opacity levels from the control dashboard.

### Preparation
1. **Model Installation**:
   - Launch the application, click the Settings button, and go to the OCR tab.
   - For MangaOCR or Built-in PaddleOCR, if the files are not detected locally, a download option and status prompt will be shown.
   - Click the Download button to automatically download and extract files into the models/manga-ocr/ or models/ppocr/ folder.
2. **GPU Optimization**:
   - NVIDIA GPUs: Install the latest graphics drivers, download the [CUDA Toolkit](https://developer.nvidia.com/cuda-downloads), and enable TensorRT acceleration settings inside the ONNX configuration panel.
   - Generic platforms run calculations via the CPU or Microsoft DirectML APIs on Windows.
3. **API Credentials Setup**:
   - Input API keys or custom server locations in the Settings > Translation panel:
     - Google Translate: Available out of the box.
     - Gemini: Obtain developer keys from [Google AI Studio](https://aistudio.google.com/).
     - Groq: Generate keys at the [Groq Console](https://console.groq.com/).
     - Ollama: Set up and run a local model node from [Ollama.com](https://ollama.com/).
     - OpenAI / Custom: Register credentials at [OpenAI Platform](https://platform.openai.com/) or [OpenRouter](https://openrouter.ai/).

### Usage Instructions
1. Run the ktranslator.exe executable to launch the application.
2. Click the gear icon on the interface to access configuration panels.
3. In the OCR tab, choose the source language, capture style (game, manga, or document), and the target character recognition engine.
4. In the Translation tab, choose your target translation endpoint, enter API credentials, and select target language settings.
5. Go back to the main layout, click Add Region, and draw a bounding box over the window or application screen you want to monitor.
6. Click Start Translate to start frame acquisition and text processing.
7. To project translated phrases directly over the image, enable Overlay Mode and customize colors and size variables.

---

### Tech Stack
- **Language:** Rust (edition 2021)
- **UI Framework:** egui
- **ML Runtime:** ONNX Runtime (ort) with CUDA, TensorRT, and DirectML support
- **OCR:** Manga-OCR (Vision Encoder-Decoder) and PaddleOCR v4
- **NLP:** wordninja, Thai word segmentation logic

### โครงการและข้อมูลอ้างอิงที่ใช้งาน (References)
โปรแกรม KTranslator V2 มีการใช้งานโมเดล รันไทม์ และชุดข้อมูลจากโครงการดังต่อไปนี้:
- **egui / eframe**: เฟรมเวิร์กสำหรับสร้างอินเตอร์เฟซผู้ใช้แบบเนทีฟเขียนด้วยภาษา Rust พัฒนาโดย Emil Ernerfeldt
- **ONNX Runtime (ort crate)**: รันไทม์ประสิทธิภาพสูงสำหรับโมเดลปัญญาประดิษฐ์ พัฒนาโดย Microsoft
- **PaddleOCR**: ชุดโมเดลตรวจจับและจำแนกอักษร (OCR) ประสิทธิภาพสูง พัฒนาโดย PaddlePaddle (Baidu)
- **oar-ocr**: ไลบรารีสำหรับรันโมเดล PaddleOCR และ Manga-OCR บน ONNX Runtime ในภาษา Rust
- **Manga-OCR**: โมเดลสแกนตัวหนังสือภาษาญี่ปุ่นแนวตั้งสำหรับมังงะโดยเฉพาะ พัฒนาโดย Kha-Lai อ้างอิงชุดข้อมูลและรูปแบบจากโครงการ Manga109
- **wordninja**: อัลกอริทึมแยกคำภาษาอังกฤษประมวลผลข้อมูลจากความถี่คำใน Wikipedia พัฒนาโดย Derek Anderson
- **dxgcap / screenshots**: ไลบรารีสำหรับจับภาพหน้าจอคอมพิวเตอร์ผ่าน Windows Desktop Duplication API

### References and Acknowledgements
KTranslator V2 utilizes models, runtimes, and libraries from the following projects:
- **egui / eframe**: Graphical user interface framework for Rust, created by Emil Ernerfeldt.
- **ONNX Runtime (ort crate)**: A cross-platform machine learning model accelerator developed by Microsoft.
- **PaddleOCR**: Deep learning optical character recognition toolkits and model suites developed by PaddlePaddle (Baidu).
- **oar-ocr**: A Rust wrapper library enabling PaddleOCR and Manga-OCR execution via ONNX Runtime.
- **Manga-OCR**: A specialized Japanese OCR engine developed by Kha-Lai, utilizing datasets from the Manga109 project.
- **wordninja**: An English text segmenter based on Wikipedia unigram frequencies, developed by Derek Anderson.
- **dxgcap / screenshots**: Screen capturing libraries utilizing the Windows Desktop Duplication API.

### License
Copyright (c) 2026 Supawit Kaennak [GPL v3.0](LICENSE). All rights reserved.
