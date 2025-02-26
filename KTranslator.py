import cv2
import pytesseract
from deep_translator import GoogleTranslator
from PIL import ImageGrab
import numpy as np
import threading
import time
import csv
import os
from tkinter import Tk, Label, StringVar, Button, Frame, ttk, Scrollbar, Text

# ตั้งค่า pytesseract
pytesseract.pytesseract.tesseract_cmd = r"C:\Program Files\Tesseract-OCR\tesseract.exe"

# ตัวแปรสำหรับการ Crop
crop_rect = [0, 0, 0, 0]  # [x1, y1, x2, y2]
dragging = False  # สถานะการลาก
resize_factor = 0.6  # อัตราการย่อขนาดภาพ (50%) ของเวลากด select แล้ว crop หน้าจอ

def select_crop_area():
    global crop_rect, dragging

    # ฟังก์ชันสำหรับ Mouse Event
    def draw_rectangle(event, x, y, flags, param):
        global crop_rect, dragging, temp_image

        # คำนวณพิกัดจริงตามอัตราการย่อขนาด
        real_x, real_y = int(x / resize_factor), int(y / resize_factor)

        if event == cv2.EVENT_LBUTTONDOWN:  # เริ่มลาก
            crop_rect[0], crop_rect[1] = real_x, real_y
            dragging = True
        elif event == cv2.EVENT_MOUSEMOVE and dragging:  # ขณะลาก
            temp_image = resized_image.copy()
            cv2.rectangle(temp_image, (int(crop_rect[0] * resize_factor), int(crop_rect[1] * resize_factor)),
                          (x, y), (0, 255, 0), 2)
            cv2.imshow("Select Area", temp_image)
        elif event == cv2.EVENT_LBUTTONUP:  # ปล่อยคลิกซ้าย
            crop_rect[2], crop_rect[3] = real_x, real_y
            dragging = False
            cv2.destroyWindow("Select Area")

    # จับภาพหน้าจอ
    screen = ImageGrab.grab()
    image = np.array(screen)
    image = cv2.cvtColor(image, cv2.COLOR_RGB2BGR)

    # ย่อขนาดภาพสำหรับแสดงผล
    resized_image = cv2.resize(image, (int(image.shape[1] * resize_factor), int(image.shape[0] * resize_factor)))
    temp_image = resized_image.copy()

    # เปิดหน้าต่างให้ผู้ใช้เลือกพื้นที่
    cv2.imshow("Select Area", temp_image)
    cv2.setMouseCallback("Select Area", draw_rectangle)
    cv2.waitKey(0)  # รอจนกว่าจะปิดหน้าต่าง

# ฟังก์ชันในการโหลดภาษาจากไฟล์ CSV
def load_languages_from_csv(file_path):
    languages = {}
    with open(file_path, newline='', encoding='utf-8') as csvfile:
        reader = csv.reader(csvfile)
        for row in reader:
            language_name = row[0]  # ชื่อภาษา
            language_code = row[1]  # รหัสภาษา
            languages[language_name] = language_code
    return languages

# ส่วนอื่น ๆ ยังคงเหมือนเดิม
class TranslatorApp:
    def __init__(self):
        self.root = Tk()
        self.root.title("KTranslator")
        self.root.geometry("620x350")
        self.root.configure(bg="#f0f8ff")  # สีพื้นหลัง

        # ล็อคขนาดหน้าต่าง
        self.root.resizable(False, False)

        # ทำให้ซ้อนทับหน้าจออื่นได้โดยไม่พับลง always on top
        self.root.attributes('-topmost', 1)

        # โหลดภาษาจากไฟล์ CSV
        current_dir = os.path.dirname(os.path.abspath(__file__))
        self.ocr_languages = load_languages_from_csv(os.path.join(current_dir, "languages.csv"))
        self.translation_languages = self.ocr_languages  # ใช้ภาษาจากไฟล์เดียวกันสำหรับการแปล

        # สร้างกรอบ UI
        self.main_frame = Frame(self.root, bg="#f0f8ff")
        self.main_frame.pack(pady=20)

        # กรอบสำหรับตัวเลือกภาษา OCR และการแปลภาษา
        lang_frame = Frame(self.main_frame, bg="#f0f8ff")
        lang_frame.pack(pady=10)

        # ตัวเลือกภาษา OCR
        self.selected_ocr_language = StringVar(self.root)
        self.selected_ocr_language.set(list(self.ocr_languages.values())[0])  # ตั้งค่าเริ่มต้น
        Label(lang_frame, text="OCR Language:", font=("Arial", 12), bg="#f0f8ff", fg="#333333").grid(row=0, column=0, padx=5)
        ocr_menu = ttk.Combobox(lang_frame, textvariable=self.selected_ocr_language, values=list(self.ocr_languages.values()), state="readonly", height=10)

        ocr_menu.grid(row=0, column=1, padx=5)

        # ตัวเลือกภาษาสำหรับการแปล
        self.selected_language = StringVar(self.root)
        self.selected_language.set(list(self.translation_languages.values())[0])  # ตั้งค่าเริ่มต้น
        Label(lang_frame, text="Translation Language:", font=("Arial", 12), bg="#f0f8ff", fg="#333333").grid(row=0, column=2, padx=5)
        translation_menu = ttk.Combobox(lang_frame, textvariable=self.selected_language, values=list(self.translation_languages.values()), state="readonly", height=10)

        translation_menu.grid(row=0, column=3, padx=5)

        # สร้างกรอบสำหรับการแสดงข้อความแปลพร้อม scroll bar
        self.text_frame = Frame(self.main_frame, bg="#f0f8ff")
        self.text_frame.pack(pady=5)

        self.scrollbar = Scrollbar(self.text_frame, orient="vertical")
        self.scrollbar.pack(side="right", fill="y")

        self.result_text = Text(self.text_frame, wrap="word", font=("Arial", 12), bg="#ffffff", fg="#555555",
                                yscrollcommand=self.scrollbar.set, width=63, height=11, state="disabled")
        self.result_text.pack(side="left", fill="both", expand=True)

        self.scrollbar.config(command=self.result_text.yview)

        # กรอบสำหรับปุ่ม
        button_frame = Frame(self.main_frame, bg="#f0f8ff")
        button_frame.pack(pady=10)

        # ปุ่มเลือกพื้นที่ Crop
        self.select_area_button = Button(button_frame, text="Select Area", command=select_crop_area, bg="#4682b4", fg="white", font=("Arial", 12), width=20)
        self.select_area_button.grid(row=0, column=0, padx=5)

        # ปุ่มเริ่มการแปล
        self.start_button = Button(button_frame, text="Start Translation", command=self.start_translation, bg="#32cd32", fg="white", font=("Arial", 12), width=20)
        self.start_button.grid(row=0, column=1, padx=5)

        # ปุ่มหยุดการแปล
        self.stop_button = Button(button_frame, text="Stop Translation", command=self.stop_translation, bg="#dc143c", fg="white", font=("Arial", 12), width=20)
        self.stop_button.grid(row=0, column=2, padx=5)

        # ใช้ Event ในการควบคุมการหยุดเธรด
        self.stop_event = threading.Event()

        self.root.mainloop()

    def start_translation(self):
        self.result_text.config(state="normal")  # เปิดให้แก้ไขข้อความได้ในขณะแปล
        self.stop_event.clear()  # Reset stop event
        threading.Thread(target=self.translate_loop).start()

    def stop_translation(self):
        self.stop_event.set()  # Set stop event
        self.result_text.config(state="disabled")  # ทำให้เป็น Read-Only เมื่อหยุดการแปล


    def stop_translation(self):
        self.stop_event.set()  # Set stop event
        self.result_text.config(state="disabled")  # ทำให้เป็น Read-Only เมื่อหยุดการแปล

    def update_translated_text(self, text):
        """ฟังก์ชันอัปเดตข้อความแปล"""
        self.result_text.config(state="normal")  # เปิดให้แก้ไขชั่วคราว
        self.result_text.delete("1.0", "end")  # ลบข้อความเก่า
        self.result_text.insert("1.0", f"Translated Text:\n{text}")  # แทรกข้อความแปลใหม่
        self.result_text.config(state="disabled")  # ตั้งเป็น Read-Only

    def translate_loop(self):
        global crop_rect
        while not self.stop_event.is_set():  # ทำงานจนกว่าจะมีการหยุด
            if crop_rect[2] > crop_rect[0] and crop_rect[3] > crop_rect[1]:
                img = ImageGrab.grab(bbox=tuple(crop_rect))
                img_np = np.array(img)
                ocr_language = self.selected_ocr_language.get()
                print(f"OCR Language: {ocr_language}")  # ดีบั๊ก: แสดงภาษาที่เลือก
                text = pytesseract.image_to_string(img_np, lang=ocr_language)
                print(f"Detected Text: {text}")  # ดีบั๊ก: แสดงข้อความที่ OCR ดึงมาได้
            if text.strip():
                target_language = self.selected_language.get()
                print(f"Translating to: {target_language}")  # ดีบั๊ก: แสดงภาษาที่แปล
                translated_text = GoogleTranslator(source='auto', target=target_language).translate(text)
                self.update_translated_text(translated_text)  # อัปเดตข้อความแปล
            else:
                self.update_translated_text("No text detected")  # ไม่มีข้อความที่ถูกตรวจจับ
        time.sleep(1)

def update_translated_text(self, text):
    """ฟังก์ชันอัปเดตข้อความแปล"""
    print(f"Updating Translated Text: {text}")  # ดีบั๊ก: แสดงข้อความที่จะแสดงใน UI
    self.result_text.config(state="normal")  # เปิดให้แก้ไขชั่วคราว
    self.result_text.config(state="disabled")  # ตั้งเป็น Read-Only

# เรียกโปรแกรม
if __name__ == "__main__":
    TranslatorApp()
