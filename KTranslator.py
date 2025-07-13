import cv2
import pytesseract
from deep_translator import GoogleTranslator
import os
from PIL import ImageGrab
import numpy as np
import threading
import time
import csv
import os
from tkinter import Tk, Label, StringVar, Button, Frame, ttk, Scrollbar, Text, Canvas

# ตั้งค่า pytesseract
import os.path

# Import pythainlp for Thai language processing
from pythainlp.tokenize import word_tokenize

default_tesseract_path = r"C:\Program Files\Tesseract-OCR\tesseract.exe"
if os.path.exists(default_tesseract_path):
    pytesseract.pytesseract.tesseract_cmd = default_tesseract_path
else:
    print(f"Warning: Tesseract executable not found at {default_tesseract_path}. Please check the path.")

# ฟังก์ชันในการโหลดภาษาจากไฟล์ CSV
def load_languages_from_csv(file_path):
    languages = {}
    try:
        with open(file_path, newline='', encoding='utf-8') as csvfile:
            reader = csv.reader(csvfile)
            for row in reader:
                language_name = row[0]  # ชื่อภาษา
                language_code = row[1]  # รหัสภาษา
                languages[language_name] = language_code
    except FileNotFoundError:
        print(f"Error: Language file {file_path} not found.")
    except Exception as e:
        print(f"Error reading language file {file_path}: {e}")
    return languages

# ส่วนอื่น ๆ ยังคงเหมือนเดิม
class TranslatorApp:
    def __init__(self):
        self.root = Tk()
        self.root.title("KTranslator")
        self.root.geometry("620x700")
        self.root.configure(bg="#f0f8ff")  # สีพื้นหลัง

        # ล็อคขนาดหน้าต่าง
        self.root.resizable(False, False)

        # ทำให้ซ้อนทับหน้าจออื่นได้โดยไม่พับลง always on top
        self.root.attributes('-topmost', 1)

        # ตัวแปรสำหรับการ Crop (ย้ายเข้ามาในคลาส)
        self.crop_rect = [0, 0, 0, 0]  # [x1, y1, x2, y2]
        self.dragging = False
        self.resize_factor = 0.6
        self.crop_lock = threading.Lock()

        # ตัวแปรสำหรับ Translator เพื่อการ Re-use
        self.translator = None
        self.last_target_lang = None

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
        if self.ocr_languages:
            self.selected_ocr_language.set(list(self.ocr_languages.values())[0])  # ตั้งค่าเริ่มต้น
        else:
            self.selected_ocr_language.set('eng')  # default fallback
        Label(lang_frame, text="OCR Language:", font=("Arial", 12), bg="#f0f8ff", fg="#333333").grid(row=0, column=0, padx=5)
        ocr_menu = ttk.Combobox(lang_frame, textvariable=self.selected_ocr_language, values=list(self.ocr_languages.values()), state="readonly", height=10)

        ocr_menu.grid(row=0, column=1, padx=5)

        # ตัวเลือกภาษาสำหรับการแปล
        self.selected_language = StringVar(self.root)
        if self.translation_languages:
            self.selected_language.set(list(self.translation_languages.values())[0])  # ตั้งค่าเริ่มต้น
        else:
            self.selected_language.set('en')  # default fallback
        Label(lang_frame, text="Translation Language:", font=("Arial", 12), bg="#f0f8ff", fg="#333333").grid(row=0, column=2, padx=5)
        translation_menu = ttk.Combobox(lang_frame, textvariable=self.selected_language, values=list(self.translation_languages.values()), state="readonly", height=10)

        translation_menu.grid(row=0, column=3, padx=5)

        # Add type-ahead search for comboboxes using a factory function
        ocr_menu.bind('<KeyPress>', self._create_keypress_handler(ocr_menu, list(self.ocr_languages.values())))
        translation_menu.bind('<KeyPress>', self._create_keypress_handler(translation_menu, list(self.translation_languages.values())))


        # สร้างกรอบสำหรับการแสดงข้อความแปลพร้อม scroll bar
        self.text_frame = Frame(self.main_frame, bg="#f0f8ff")
        self.text_frame.pack(pady=5)

        self.scrollbar = Scrollbar(self.text_frame, orient="vertical")
        self.scrollbar.pack(side="right", fill="y")

        self.result_text = Text(self.text_frame, wrap="word", font=("AngsanaUPC", 20), bg="#ffffff", fg="#555555",
                                yscrollcommand=self.scrollbar.set, width=63, height=15, state="disabled")
        self.result_text.pack(side="left", fill="both", expand=True)

        self.scrollbar.config(command=self.result_text.yview)

        # กรอบสำหรับปุ่ม
        button_frame = Frame(self.main_frame, bg="#f0f8ff")
        button_frame.pack(pady=10)

        # ปุ่มเลือกพื้นที่ Crop
        self.select_area_button = Button(button_frame, text="Select Area", command=self.select_crop_area, bg="#4682b4", fg="white", font=("Arial", 12), width=20)
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

    def _create_keypress_handler(self, combobox, values):
        """Creates a keypress handler for a combobox to enable type-ahead search."""
        search_string = ''
        last_key_time = 0

        def on_keypress(event):
            nonlocal search_string, last_key_time
            current_time = time.time()

            # Reset search string if more than 1 second has passed
            if current_time - last_key_time > 1:
                search_string = ''
            last_key_time = current_time

            char = event.char.lower()
            if not char.isprintable():
                return

            search_string += char
            # Find the first language that starts with the search string
            for idx, val in enumerate(values):
                if val.lower().startswith(search_string):
                    def select_idx():
                        combobox.current(idx)
                        combobox.event_generate('<Button-1>')  # Open dropdown to show selection
                    self.root.after(50, select_idx)
                    break
        return on_keypress

    def select_crop_area(self):
        """Opens a transparent overlay window to select a screen area for OCR."""
        # Create a transparent fullscreen window using Tkinter
        overlay = Tk()
        overlay.attributes('-fullscreen', True)
        overlay.attributes('-topmost', True)
        overlay.attributes('-alpha', 0.3)  # semi-transparent
        overlay.config(bg='black')

        # Variables to store crop rectangle coordinates
        self.crop_rect = [0, 0, 0, 0]
        self.dragging = False

        # Canvas for drawing rectangle
        canvas = Canvas(overlay, cursor="cross", bg='black')
        canvas.pack(fill="both", expand=True)

        def on_button_press(event):
            self.crop_rect[0] = event.x
            self.crop_rect[1] = event.y
            self.crop_rect[2] = event.x
            self.crop_rect[3] = event.y
            self.dragging = True
            canvas.delete("rect")
            canvas.create_rectangle(self.crop_rect[0], self.crop_rect[1], self.crop_rect[2], self.crop_rect[3], outline='green', width=2, tag="rect")

        def on_move_press(event):
            if not self.dragging:
                return
            self.crop_rect[2] = event.x
            self.crop_rect[3] = event.y
            canvas.delete("rect")
            canvas.create_rectangle(self.crop_rect[0], self.crop_rect[1], self.crop_rect[2], self.crop_rect[3], outline='green', width=2, tag="rect")

        def on_button_release(event):
            self.dragging = False
            # Close the overlay window after selection
            overlay.destroy()

        canvas.bind("<ButtonPress-1>", on_button_press)
        canvas.bind("<B1-Motion>", on_move_press)
        canvas.bind("<ButtonRelease-1>", on_button_release)

        overlay.mainloop()

    def start_translation(self):
        self.result_text.config(state="normal")  # เปิดให้แก้ไขข้อความได้ในขณะแปล
        self.stop_event.clear()  # Reset stop event
        threading.Thread(target=self.translate_loop, daemon=True).start()

    def stop_translation(self):
        self.stop_event.set()  # Set stop event
        self.result_text.config(state="disabled")  # ทำให้เป็น Read-Only เมื่อหยุดการแปล

    def update_translated_text(self, text):
        """ฟังก์ชันอัปเดตข้อความแปล"""
        self.result_text.config(state="normal")  # เปิดให้แก้ไขชั่วคราว
        self.result_text.delete("1.0", "end")  # ลบข้อความเก่า
        # Insert text with word wrapping and ensure no truncation
        self.result_text.insert("1.0", f"Translated Text:\n{text}\n")
        self.result_text.config(state="disabled")  # ตั้งเป็น Read-Only

    def translate_loop(self):
        while not self.stop_event.is_set():  # ทำงานจนกว่าจะมีการหยุด
            try:
                with self.crop_lock:
                    current_crop = self.crop_rect.copy()

                text = ""
                if current_crop[2] > current_crop[0] and current_crop[3] > current_crop[1]:
                    img = ImageGrab.grab(bbox=tuple(current_crop))
                    img_np = np.array(img)
                    ocr_language = self.selected_ocr_language.get()
                    text = pytesseract.image_to_string(img_np, lang=ocr_language)

                if text.strip():
                    target_language = self.selected_language.get()

                    # Recreate translator only if target language changes for efficiency
                    if self.translator is None or self.last_target_lang != target_language:
                        self.translator = GoogleTranslator(source='auto', target=target_language)
                        self.last_target_lang = target_language
                        print(f"Translator target set to: {target_language}")

                    translated_text = self.translator.translate(text)
                    # Post-process Thai translation for better grammar if target is Thai
                    if self.last_target_lang == 'th':
                        # Tokenize Thai text and join with spaces (simple post-processing)
                        tokens = word_tokenize(translated_text, engine='newmm')
                        translated_text = ' '.join(tokens)
                    # Schedule GUI update on the main thread (Thread-safe)
                    self.root.after(0, lambda t=translated_text: self.update_translated_text(t))
                else:
                    self.root.after(0, lambda: self.update_translated_text("No text detected"))

            except Exception as e:
                error_message = f"An error occurred: {e}"
                print(error_message)
                self.root.after(0, lambda: self.update_translated_text(error_message))
            finally:
                # Wait before the next loop
                time.sleep(1)

# เรียกโปรแกรม
if __name__ == "__main__":
    TranslatorApp()
