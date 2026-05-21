# KTranslator V2
[ภาษาไทย (Thai)](#thai) | [English](#english)

---
<img width="2202" height="1317" alt="{14E20CD6-4796-44B9-A21E-F490F769EF4C}" src="https://github.com/user-attachments/assets/ace93af7-76d5-4111-a88a-0465e0e346cc" /><br>
<img width="2245" height="1169" alt="{CB32388A-95A4-4669-BD83-F3F1FD8C83FF}" src="https://github.com/user-attachments/assets/800453ff-888a-4630-8f55-4d9b6b8de1ce" /><br>
<img width="2562" height="1440" alt="{6E9BAEBF-E11A-4561-8FF7-583830210EDA}" src="https://github.com/user-attachments/assets/cee5db85-4186-4c21-8608-c5c676a4ff01" /><br>

<a name="thai"></a>
## ภาษาไทย (Thai)

โปรแกรมแปลภาษาจากการจับภาพหน้าจอ (Screen Translator) เขียนด้วยภาษา Rust ใช้เฟรมเวิร์ก egui สำหรับหน้าจอติดต่อผู้ใช้ และใช้ ONNX Runtime ในการรันโมเดลปัญญาประดิษฐ์ (AI) 

### ความสามารถของโปรแกรม
- **การเลือกพื้นที่เพื่อจับภาพหน้าจอ**: ผู้ใช้สามารถกำหนดขอบเขตพื้นที่หน้าจอเพื่อจับภาพข้อความ โดยสามารถย้ายตำแหน่ง ปรับขนาดกล่องเลือกได้แบบเรียลไทม์ และมีโหมดแสดงคำแปลทับตำแหน่งข้อความเดิม (Overlay Mode)
- **ตัวเลือกเอนจิน OCR (ONNX Runtime)**:
  - Manga-OCR: โมเดลสแกนตัวอักษรภาษาญี่ปุ่นที่ครอบคลุมการอ่านทั้งแนวตั้งและแนวนอน
  - PaddleOCR v4 (Mobile): โมเดลสแกนตัวอักษรรุ่นขนาดเล็ก
  - PaddleOCR v4 (Server): โมเดลรุ่นมาตรฐานสำหรับประมวลผลข้อความที่มีโครงสร้างและฟอนต์ที่ซับซ้อน
  - Windows OCR: ระบบรู้จำอักขระผ่าน API บิวท์อินของระบบปฏิบัติการ Windows
- **การประมวลผลด้วย GPU (Hardware Acceleration)**: รองรับการเชื่อมต่อกับไดรเวอร์การ์ดจอ NVIDIA CUDA, TensorRT และ DirectML บนระบบปฏิบัติการ Windows 
- **การวิเคราะห์การจัดหน้า (Layout Analysis)**:
  - การตรวจจับกรอบคำพูด (Speech Bubble Detection): ค้นหากรอบคำพูดด้วยโมเดลตรวจจับวัตถุ YOLO 
  - การเรียงลำดับคำสแกน (Smart Sorting): จัดทิศทางของข้อความให้อ่านจากขวาไปซ้าย (RTL) หรือจากบนลงล่างตามพิกัดของข้อความ
- **การประมวลผลข้อความ (Text Processing)**:
  - ระบบคัดกรองขยะ (Garbage Filtering): ลบตัวอักษรที่ซ้ำซ้อน บรรทัดว่าง หรือกลุ่มสัญลักษณ์พิเศษก่อนส่งคำแปล
  - การแปลงภาษาญี่ปุ่น: จัดการรูปแบบอักขระคานะ ลบคำอ่านฟูริกานะ (Furigana) 
  - การตัดบรรทัดภาษาไทย: แทรกช่องว่างขนาดศูนย์ (Zero Width Space) ในผลลัพธ์ภาษาไทยเพื่อให้เบราว์เซอร์และ UI จัดการตัดบรรทัดได้
  - การแยกคำภาษาอังกฤษ: ใช้อัลกอริทึม wordninja ในการแยกตัวอักษรภาษาอังกฤษที่ติดกันออกเป็นคำ
- **ระบบแปลภาษา (Translation Providers)**:
  - Google Translate: การแปลผ่านหน้าเว็บ
  - Gemini API: เชื่อมต่อผ่านรหัส API Key จาก Google AI Studio
  - Groq API: เชื่อมต่อผ่าน API ไปยังโมเดลเช่น Llama หรือ Gemma
  - Ollama: เชื่อมต่อกับเซิร์ฟเวอร์ Ollama ในเครื่องเพื่อรันโมเดลภาษาในแบบออฟไลน์
  - OpenAI / Custom: ส่งคำขอไปยัง API ที่มีโครงสร้างแบบ OpenAI เช่น OpenRouter หรือ DeepSeek
- **การจัดการและการตั้งค่าโปรแกรม**:
  - แคชข้อความ (Translation Cache): บันทึกผลการแปลและ OCR ในหน่วยความจำ เพื่อนำมาแสดงซ้ำหากภาพหน้าจอยังคงเป็นข้อความเดิม 
  - การปรับแต่งสไตล์: ตั้งค่าสีตัวอักษร ขนาดฟอนต์ สีพื้นหลัง และความโปร่งแสงในโหมด Overlay

### การติดตั้งและการเตรียมความพร้อม
1. **การติดตั้งโมเดล (Model Installation)**:
   - ไปที่เมนู Settings (ไอคอนฟันเฟือง) เลือกแท็บ OCR
   - หากเลือกระบบ MangaOCR, Built-in PaddleOCR หรือ Bubble YOLO โปรแกรมจะตรวจสอบไฟล์โมเดลในเครื่อง หากไม่มีไฟล์ จะปรากฏปุ่ม "Download" 
   - คลิกปุ่ม Download เพื่อโหลดและคลายซิปไฟล์ลงโฟลเดอร์ `models/` โดยอัตโนมัติ
2. **การรันด้วย GPU**:
   - หากต้องการรันผ่านการ์ดจอ NVIDIA ให้ติดตั้งไดรเวอร์กราฟิกล่าสุด และ [CUDA Toolkit](https://developer.nvidia.com/cuda-downloads) 
   - หากเลือกใช้ TensorRT จำเป็นต้องตั้งค่าความเข้ากันได้ของระบบในส่วน ONNX เพิ่มเติม
   - หากไม่ใช้ CUDA โปรแกรมจะทำงานผ่าน CPU หรือ Microsoft DirectML
3. **การรับคีย์เชื่อมต่อบริการแปลภาษา (API Setup)**:
   - นำคีย์ API ใส่ในหน้า Settings > Translation:
     - Google Translate: ใช้งานได้ทันทีโดยไม่ต้องใส่คีย์
     - Gemini: ขอคีย์ที่ [Google AI Studio](https://aistudio.google.com/)
     - Groq: ขอคีย์ที่ [Groq Console](https://console.groq.com/)
     - Ollama: ดาวน์โหลดเซิร์ฟเวอร์และโมเดลจาก [Ollama.com](https://ollama.com/)
     - OpenAI / Custom: ขอคีย์จาก [OpenAI Platform](https://platform.openai.com/) หรือ [OpenRouter](https://openrouter.ai/)

### วิธีใช้งาน
1. รันไฟล์ `ktranslator.exe` เพื่อเปิดโปรแกรม
2. คลิกปุ่มตั้งค่า (ฟันเฟือง) เพื่อเลือกภาษาต้นทาง-ปลายทาง, เอนจิน OCR และตั้งค่าผู้ให้บริการแปลภาษา
3. กดปุ่ม **Add Region** แล้วใช้เมาส์ลากครอบพื้นที่บนหน้าจอที่ต้องการ
4. กดปุ่ม **Start Translate** เพื่อเริ่มรอบการจับภาพและประมวลผล
5. หากต้องการให้ข้อความแสดงทับตำแหน่งเดิม ให้กดสวิตช์เปิดใช้งาน **Overlay Mode**
6. ข้อความจะอัปเดตอัตโนมัติเมื่อเนื้อหาบนหน้าจอในกรอบมีการเปลี่ยนแปลง

---

<a name="english"></a>
## English

KTranslator V2 is a screen capture translation utility written in Rust. It utilizes the egui framework for its graphical interface and ONNX Runtime to execute artificial intelligence models.

### Capabilities
- **Region-Based Capture**: Users can draw bounding boxes on the screen to specify the capture area. These boxes can be resized and moved in real time. The software includes an Overlay Mode to render translated text directly over the original screen contents.
- **OCR Engine Selection (ONNX Runtime)**:
  - Manga-OCR: A model for scanning Japanese text, covering both vertical and horizontal writing formats.
  - PaddleOCR v4 (Mobile): A scaled-down model variant for lower memory footprint.
  - PaddleOCR v4 (Server): A standard model variant for processing complex structures and fonts.
  - Windows OCR: Text recognition utilizing the built-in Windows API.
- **Hardware Acceleration**: Integrates with NVIDIA CUDA, TensorRT, and DirectML APIs on Windows to route ONNX computations through the GPU.
- **Layout Analysis**:
  - Speech Bubble Detection: Uses a YOLO object detection model to locate speech bubbles.
  - Smart Sorting: Sorts recognized text boxes according to spatial coordinates (e.g., Right-to-Left or Top-to-Bottom).
- **Text Processing**:
  - Garbage Filtering: Removes repeating characters, empty lines, and excessive symbolic characters before passing text to the translator.
  - Japanese Normalization: Formats kana characters and removes furigana phonetic guides.
  - Thai Word Wrap: Inserts Zero Width Spaces into Thai strings to allow correct line breaking on UI rendering.
  - English Word Segmentation: Uses the wordninja algorithm to split combined character sequences into distinct words.
- **Translation Providers**:
  - Google Translate: Web-based translation implementation.
  - Gemini API: Connects using API keys from Google AI Studio.
  - Groq API: Connects to Llama or Gemma inference endpoints via Groq.
  - Ollama: Targets a locally hosted Ollama server for offline language model execution.
  - OpenAI / Custom: Sends requests to APIs implementing the OpenAI interface format, such as OpenRouter or DeepSeek.
- **Management and Configuration**:
  - Translation Cache: Records OCR and translation outputs in memory, preventing duplicate API calls when the target screen content remains static.
  - Styling Customization: Configuration of font colors, sizes, background colors, and overlay opacity.

### Installation and Setup
1. **Model Installation**:
   - Go to Settings (gear icon) and select the OCR tab.
   - If using MangaOCR, Built-in PaddleOCR, or Bubble YOLO, the program will check for local model files. If they are missing, a "Download" button is displayed.
   - Click Download to automatically fetch and extract the files into the `models/` directory.
2. **GPU Execution**:
   - To utilize an NVIDIA GPU, install the latest graphics drivers and the [CUDA Toolkit](https://developer.nvidia.com/cuda-downloads).
   - Using TensorRT requires additional dependency configurations in the ONNX environment.
   - If CUDA is not used, the program defaults to the CPU or Microsoft DirectML.
3. **API Credentials Setup**:
   - Enter API credentials in Settings > Translation:
     - Google Translate: Operates without a key.
     - Gemini: Obtain an API key from [Google AI Studio](https://aistudio.google.com/).
     - Groq: Obtain an API key from [Groq Console](https://console.groq.com/).
     - Ollama: Download the server and models from [Ollama.com](https://ollama.com/).
     - OpenAI / Custom: Obtain an API key from [OpenAI Platform](https://platform.openai.com/) or [OpenRouter](https://openrouter.ai/).

### Usage Instructions
1. Run the `ktranslator.exe` executable.
2. Click the Settings button (gear icon) to configure source and target languages, select the OCR engine, and choose the translation provider.
3. Click **Add Region** and click-and-drag to define the target area on the screen.
4. Click **Start Translate** to begin the capture loop.
5. Toggle **Overlay Mode** to project the translated text directly onto the bounding box area.
6. The translations will automatically update whenever the content within the bounding box changes.

---

### Tech Stack
- **Language:** Rust (edition 2021)
- **UI Framework:** egui
- **ML Runtime:** ONNX Runtime (ort) with CUDA, TensorRT, and DirectML support
- **OCR:** Manga-OCR (Vision Encoder-Decoder) and PaddleOCR v4
- **NLP:** wordninja, Thai word segmentation logic

### โครงการและข้อมูลอ้างอิงที่ใช้งาน (References)
โปรแกรม KTranslator V2 มีการเรียกใช้ชุดข้อมูล เครื่องมือ และโมเดลจากโครงการต่อไปนี้:
- **egui / eframe**: เฟรมเวิร์กสำหรับสร้างอินเตอร์เฟซผู้ใช้เขียนด้วยภาษา Rust พัฒนาโดย Emil Ernerfeldt
- **ONNX Runtime (ort crate)**: รันไทม์ข้ามแพลตฟอร์มสำหรับรันโมเดลปัญญาประดิษฐ์ พัฒนาโดย Microsoft
- **PaddleOCR**: ชุดโมเดลตรวจจับและจำแนกอักษร พัฒนาโดย PaddlePaddle (Baidu)
- **oar-ocr**: ไลบรารีอ้างอิงและประมวลผลสำหรับนำโมเดล PaddleOCR และ Manga-OCR มารันบน ONNX ในภาษา Rust
- **Manga-OCR**: โมเดลสแกนตัวหนังสือภาษาญี่ปุ่น พัฒนาโดย Kha-Lai อ้างอิงชุดข้อมูลโครงสร้างจากโครงการ Manga109
- **wordninja**: โค้ดสำหรับแยกคำภาษาอังกฤษประมวลผลจากความถี่คำใน Wikipedia พัฒนาโดย Derek Anderson
- **dxgcap / screenshots**: ไลบรารีสำหรับจับภาพหน้าจอผ่าน Windows Desktop Duplication API

### References and Acknowledgements
KTranslator V2 utilizes tools, models, and libraries from the following projects:
- **egui / eframe**: Graphical user interface framework for Rust, created by Emil Ernerfeldt.
- **ONNX Runtime (ort crate)**: A cross-platform machine learning accelerator developed by Microsoft.
- **PaddleOCR**: Deep learning optical character recognition toolkits developed by PaddlePaddle (Baidu).
- **oar-ocr**: A Rust wrapper library enabling PaddleOCR and Manga-OCR execution via ONNX Runtime.
- **Manga-OCR**: A Japanese OCR model developed by Kha-Lai, utilizing datasets from the Manga109 project.
- **wordninja**: An English text segmenter based on Wikipedia unigram frequencies, developed by Derek Anderson.
- **dxgcap / screenshots**: Screen capturing libraries utilizing the Windows Desktop Duplication API.

### License
Copyright (c) 2026 Supawit Kaennak [GPL v3.0](LICENSE). All rights reserved.
