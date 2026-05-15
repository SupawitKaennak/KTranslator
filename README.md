# KTranslator V2

[ภาษาไทย (Thai)](#thai) | [English](#english)

---

<a name="thai"></a>
## ภาษาไทย (Thai)

โปรแกรมแปลภาษาจากการจับภาพหน้าจอ (Screen Translator) ประสิทธิภาพสูง เขียนด้วยภาษา Rust เน้นการประมวลผลออฟไลน์ที่รวดเร็ว แม่นยำ และรองรับการปรับแต่งที่ยืดหยุ่น

### สิ่งที่โปรแกรมทำได้
- **Advanced OCR Engines (Native ONNX):**
  - **Manga-OCR:** ใช้ AI ตรวจจับและอ่านภาษาญี่ปุ่นแนวตั้งในมังงะโดยเฉพาะ (อ้างอิงฐานข้อมูล Manga109)
  - **PaddleOCR v4 (Mobile):** เน้นความเร็วสูงสุด กินทรัพยากรต่ำ เหมาะสำหรับคอมพิวเตอร์สเปคทั่วไป
  - **PaddleOCR v4 (Server):** เน้นความแม่นยำสูงสุดสำหรับฟอนต์ในเกมหรือเอกสารที่ซับซ้อน
- **Hardware Acceleration:** ปลดล็อคพลังการ์ดจอผ่าน **NVIDIA CUDA, TensorRT** และ **DirectML** ลดโหลด CPU ลงสูงสุดถึง 90%
- **Intelligent Layout Analysis:**
  - **Bubble Detection:** ตรวจจับกรอบคำพูดมังงะอัตโนมัติ
  - **Smart Sorting:** จัดเรียงลำดับการอ่านแบบ Column-Major (ขวาไปซ้าย) รองรับมังงะหน้าคู่
- **Text Processing:** 
  - **Thai Zero Width Space:** ระบบช่วยตัดบรรทัดภาษาไทยให้สวยงาม
  - **English Word Segmentation:** แยกคำภาษาอังกฤษที่ติดกันด้วย `wordninja`
- **Dynamic Overlay:** แสดงผลคำแปลทับตำแหน่งเดิม พร้อมปรับสี พื้นหลัง ขนาดฟอนต์ และความโปร่งใสได้แบบ Real-time

### การเตรียมความพร้อมก่อนใช้งาน
1. **ติดตั้งโมเดล (Model Installation):**
   - ไปที่หน้า **Settings > Model Installation Center**
   - กดปุ่มดาวน์โหลดโมเดลที่ต้องการ (Manga-OCR หรือ PaddleOCR) โปรแกรมจะจัดการโฟลเดอร์ให้โดยอัตโนมัติ
2. **การเร่งความเร็วด้วย GPU (สำหรับ NVIDIA):**
   - ติดตั้งไดรเวอร์การ์ดจอเวอร์ชันล่าสุด
   - (แนะนำ) ติดตั้ง [CUDA Toolkit](https://developer.nvidia.com/cuda-downloads) เพื่อประสิทธิภาพสูงสุด
3. **สมัครใช้งาน API สำหรับการแปล:**
   - **Google Translate:** ใช้งานได้ฟรีทันที (ไม่ต้องมีคีย์)
   - **Gemini:** [Google AI Studio](https://aistudio.google.com/) (แนะนำ: ฟรีและเร็ว)
   - **Groq:** [Groq Console](https://console.groq.com/) (ความเร็วสูงพิเศษ)
   - **Ollama:** [Ollama.com](https://ollama.com/) (สำหรับการแปลแบบออฟไลน์ 100%)
   - **OpenAI / Custom:** สมัครที่ [OpenAI Platform](https://platform.openai.com/) หรือใช้งานผ่าน [OpenRouter](https://openrouter.ai/)

### วิธีใช้งาน
1. เปิดโปรแกรมและเข้าไปที่หน้า **Settings** (ไอคอนฟันเฟือง)
2. เลือก **OCR Engine** และ **Translation Provider** ที่ต้องการ (พร้อมกรอก API Key)
3. กดปุ่ม **Add Region** บนหน้าหลัก เพื่อลากพื้นที่บนหน้าจอที่ต้องการแปล
4. เลือกภาษาต้นทาง (From) และปลายทาง (To)
5. กดปุ่ม **Start Translate**
6. เปิด **Overlay Mode** เพื่อให้คำแปลแสดงทับข้อความเดิมในตำแหน่งที่ถูกต้อง

---

<a name="english"></a>
## English

A high-performance Screen Translator written in Rust, optimized for fast, accurate, and flexible offline/online processing.

### Key Features
- **Advanced OCR Engines (Native ONNX):**
  - **Manga-OCR:** Specialized AI for vertical Japanese text recognition in manga (Manga109 based).
  - **PaddleOCR v4 (Mobile):** Ultra-fast and lightweight, ideal for standard hardware.
  - **PaddleOCR v4 (Server):** High-precision recognition for complex game fonts and documents.
- **Hardware Acceleration:** Native support for **NVIDIA CUDA, TensorRT**, and **DirectML**, reducing CPU usage by up to 90%.
- **Intelligent Layout Analysis:**
  - **Bubble Detection:** Automated detection of manga speech bubbles.
  - **Smart Sorting:** RTL Column-Major sorting support for double-page manga spreads.
- **Advanced Text Processing:** 
  - **Thai Word Wrap:** Automated Zero Width Space injection for beautiful Thai rendering.
  - **English Segmentation:** Splits unspaced English text using `wordninja`.
- **Dynamic Overlay:** Real-time translation overlay with customizable colors, opacity, and font styles.

### Preparation
1. **Model Installation:**
   - Open **Settings > Model Installation Center**.
   - Download the required suites (Manga-OCR or PaddleOCR). The app handles all path configurations.
2. **GPU Acceleration (NVIDIA Users):**
   - Install the latest NVIDIA drivers.
   - (Recommended) Install [CUDA Toolkit](https://developer.nvidia.com/cuda-downloads) for peak performance.
3. **Translation API Setup:**
   - **Google Translate:** Free to use (No key required).
   - **Gemini:** [Google AI Studio](https://aistudio.google.com/) (Recommended: Fast & Free tier).
   - **Groq:** [Groq Console](https://console.groq.com/) (Extreme inference speed).
   - **Ollama:** [Ollama.com](https://ollama.com/) (For 100% offline local LLM translation).
   - **OpenAI / Custom:** [OpenAI Platform](https://platform.openai.com/) or [OpenRouter](https://openrouter.ai/).

### Usage Instructions
1. Launch the app and go to **Settings** (gear icon).
2. Configure your **OCR Engine** and **Translation Provider** (enter API keys where required).
3. Click **Add Region** and drag to select the capture area on your screen.
4. Set the Source (From) and Target (To) languages.
5. Click **Start Translate**.
6. Toggle **Overlay Mode** to display translations over the original text accurately.

---

### Tech Stack
- **Language:** Rust (edition 2024)
- **UI Framework:** egui
- **ML Runtime:** ONNX Runtime (ort) with CUDA/TensorRT/DirectML support
- **OCR:** Manga-OCR (Vision Encoder-Decoder) & PaddleOCR v4
- **NLP:** wordninja, Thai word segmentation logic

### License
Copyright (c) 2026 Supawit Kaennak [GPL v3.0](LICENSE). All rights reserved.
