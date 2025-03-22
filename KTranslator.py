import os
import csv
import time
import threading

import cv2
import pytesseract
from pytesseract import Output
from deep_translator import GoogleTranslator
from PIL import ImageGrab
import numpy as np

# ----- pywin32 -----
import win32api
import win32gui
import win32ui
import win32con
import ctypes

# ----- Tkinter UI -----
from tkinter import Tk, Label, StringVar, Button, Frame, ttk, Scrollbar, Text

# สร้างคลาส BLENDFUNCTION ด้วย ctypes
class BLENDFUNCTION(ctypes.Structure):
    _fields_ = [("BlendOp", ctypes.c_byte),
                ("BlendFlags", ctypes.c_byte),
                ("SourceConstantAlpha", ctypes.c_byte),
                ("AlphaFormat", ctypes.c_byte)]

# ======================
# 1) คลาส OverlayWindow
# ======================
class OverlayWindow:
    """
    สร้างหน้าต่างโปร่งใส (Transparent + Click-Through) แบบ Layered Window
    เพื่อวาดข้อความทับหน้าจอได้จริง ๆ โดยไม่กิน event เมาส์
    """
    def __init__(self, width=600, height=200, x=100, y=100):
        self.width = width
        self.height = height
        self.x = x
        self.y = y

        # ลงทะเบียนคลาสหน้าต่าง
        hInstance = win32api.GetModuleHandle(None)
        className = "OverlayWindowClass"

        wndClass = win32gui.WNDCLASS()
        wndClass.hInstance = hInstance
        wndClass.lpszClassName = className
        wndClass.lpfnWndProc = self.wnd_proc
        wndClass.style = win32con.CS_HREDRAW | win32con.CS_VREDRAW
        wndClass.hCursor = win32gui.LoadCursor(None, win32con.IDC_ARROW)
        wndClass.hbrBackground = win32con.COLOR_WINDOW
        wndClass.lpszMenuName = ""  # เปลี่ยนจาก None เป็น string ว่าง

        self.atom = win32gui.RegisterClass(wndClass)

        # สร้างหน้าต่างแบบ Layered + Transparent + ToolWindow
        ex_style = (win32con.WS_EX_LAYERED |
                    win32con.WS_EX_TRANSPARENT |
                    win32con.WS_EX_TOOLWINDOW)

        style = win32con.WS_POPUP  # ไม่มีขอบ

        self.hwnd = win32gui.CreateWindowEx(
            ex_style,
            self.atom,
            "OverlayWindow",
            style,
            self.x, self.y,
            self.width, self.height,
            0, 0, hInstance, None
        )

        # จัดให้อยู่บนสุด (TOPMOST) และแสดง
        win32gui.SetWindowPos(
            self.hwnd,
            win32con.HWND_TOPMOST,
            self.x, self.y,
            self.width, self.height,
            win32con.SWP_SHOWWINDOW
        )

        win32gui.UpdateWindow(self.hwnd)
        win32gui.ShowWindow(self.hwnd, win32con.SW_SHOW)

    def wnd_proc(self, hwnd, msg, wparam, lparam):
        if msg == win32con.WM_DESTROY:
            win32gui.PostQuitMessage(0)
            return 0
        return win32gui.DefWindowProc(hwnd, msg, wparam, lparam)

    def destroy(self):
        win32gui.DestroyWindow(self.hwnd)
        win32gui.UnregisterClass(self.atom, None)

    def update_overlay(self, draw_func):
        # 1) เตรียม DC หน้าจอ
        hdc_screen = win32gui.GetDC(0)
        dc_screen = win32ui.CreateDCFromHandle(hdc_screen)

        # 2) สร้าง DC ชั่วคราว (Compatible DC)
        dc_temp = dc_screen.CreateCompatibleDC()

        # 3) สร้าง Bitmap (HBITMAP) สำหรับวาด
        bmp = win32ui.CreateBitmap()
        bmp.CreateCompatibleBitmap(dc_screen, self.width, self.height)
        dc_temp.SelectObject(bmp)

        # 4) เคลียร์พื้นหลังให้เป็นโปร่งใส (ARGB=0)
        brush = win32gui.CreateSolidBrush(0x000000)
        rect = (0, 0, self.width, self.height)
        win32gui.FillRect(dc_temp.GetSafeHdc(), rect, brush)

        # 5) เรียกฟังก์ชันวาดจากภายนอก
        draw_func(dc_temp, self.width, self.height)

        # 6) ใช้ tuple สำหรับ BLENDFUNCTION
        blend = (win32con.AC_SRC_OVER, 0, 255, win32con.AC_SRC_ALPHA)

        # 7) UpdateLayeredWindow
        win32gui.UpdateLayeredWindow(
            self.hwnd,
            hdc_screen,
            (self.x, self.y),
            (self.width, self.height),
            dc_temp.GetSafeHdc(),
            (0, 0),
            0,
            blend,
            win32con.ULW_ALPHA
        )

        # 8) เคลียร์หน่วยความจำ
        dc_temp.DeleteDC()
        dc_screen.DeleteDC()
        win32gui.ReleaseDC(self.hwnd, hdc_screen)
        win32gui.DeleteObject(bmp.GetHandle())
        win32gui.DeleteObject(brush)

# ======================
# 2) ฟังก์ชันโหลด CSV ภาษาสำหรับ OCR/Translate
# ======================
def load_languages_from_csv(file_path):
    languages = {}
    with open(file_path, newline='', encoding='utf-8') as csvfile:
        reader = csv.reader(csvfile)
        for row in reader:
            language_name = row[0]  # ชื่อภาษา
            language_code = row[1]  # รหัสภาษา
            languages[language_name] = language_code
    return languages


# ======================
# 3) ฟังก์ชันเลือกพื้นที่ Crop ด้วย OpenCV
# ======================
crop_rect = [0, 0, 0, 0]  # [x1, y1, x2, y2]
dragging = False
resize_factor = 0.6

def select_crop_area():
    global crop_rect, dragging

    def draw_rectangle(event, x, y, flags, param):
        global crop_rect, dragging, temp_image
        real_x, real_y = int(x / resize_factor), int(y / resize_factor)
        if event == cv2.EVENT_LBUTTONDOWN:
            crop_rect[0], crop_rect[1] = real_x, real_y
            dragging = True
        elif event == cv2.EVENT_MOUSEMOVE and dragging:
            temp_image = resized_image.copy()
            cv2.rectangle(temp_image, (int(crop_rect[0]*resize_factor), int(crop_rect[1]*resize_factor)),
                          (x, y), (0, 255, 0), 2)
            cv2.imshow("Select Area", temp_image)
        elif event == cv2.EVENT_LBUTTONUP:
            crop_rect[2], crop_rect[3] = real_x, real_y
            dragging = False
            cv2.destroyWindow("Select Area")

    # จับภาพหน้าจอ
    screen = ImageGrab.grab()
    image = np.array(screen)
    image = cv2.cvtColor(image, cv2.COLOR_RGB2BGR)

    # ย่อขนาดภาพ
    resized_image = cv2.resize(image, (int(image.shape[1]*resize_factor), int(image.shape[0]*resize_factor)))
    temp_image = resized_image.copy()

    cv2.imshow("Select Area", temp_image)
    cv2.setMouseCallback("Select Area", draw_rectangle)
    cv2.waitKey(0)


# ======================
# 4) คลาสหลัก TranslatorApp (Tkinter + Overlay)
# ======================
class TranslatorApp:
    def __init__(self):
        # ---- สร้างหน้าต่าง Tkinter ----
        self.root = Tk()
        self.root.title("KTranslator Overlay Demo")
        self.root.geometry("620x350")
        self.root.configure(bg="#f0f8ff")
        self.root.resizable(False, False)
        self.root.attributes('-topmost', 1)

        # โหลดภาษาจาก CSV (ปรับ path ให้ตรงตามไฟล์จริง)
        current_dir = os.path.dirname(os.path.abspath(__file__))
        self.ocr_languages = load_languages_from_csv(os.path.join(current_dir, "languages.csv"))
        self.translation_languages = self.ocr_languages

        # สร้างกรอบ UI
        self.main_frame = Frame(self.root, bg="#f0f8ff")
        self.main_frame.pack(pady=20)

        lang_frame = Frame(self.main_frame, bg="#f0f8ff")
        lang_frame.pack(pady=10)

        # ตัวเลือกภาษา OCR
        self.selected_ocr_language = StringVar(self.root)
        self.selected_ocr_language.set(list(self.ocr_languages.values())[0])
        Label(lang_frame, text="OCR Language:", font=("Arial", 12), bg="#f0f8ff", fg="#333333")\
            .grid(row=0, column=0, padx=5)
        ocr_menu = ttk.Combobox(lang_frame, textvariable=self.selected_ocr_language,
                                values=list(self.ocr_languages.values()), state="readonly", height=10)
        ocr_menu.grid(row=0, column=1, padx=5)

        # ตัวเลือกภาษาแปล
        self.selected_language = StringVar(self.root)
        self.selected_language.set(list(self.translation_languages.values())[0])
        Label(lang_frame, text="Translation Language:", font=("Arial", 12), bg="#f0f8ff", fg="#333333")\
            .grid(row=0, column=2, padx=5)
        translation_menu = ttk.Combobox(lang_frame, textvariable=self.selected_language,
                                        values=list(self.translation_languages.values()), state="readonly", height=10)
        translation_menu.grid(row=0, column=3, padx=5)

        # กรอบปุ่ม
        button_frame = Frame(self.main_frame, bg="#f0f8ff")
        button_frame.pack(pady=10)

        Button(button_frame, text="Select Area", command=select_crop_area,
               bg="#4682b4", fg="white", font=("Arial", 12), width=20)\
            .grid(row=0, column=0, padx=5)

        Button(button_frame, text="Start Translation", command=self.start_translation,
               bg="#32cd32", fg="white", font=("Arial", 12), width=20)\
            .grid(row=0, column=1, padx=5)

        Button(button_frame, text="Stop Translation", command=self.stop_translation,
               bg="#dc143c", fg="white", font=("Arial", 12), width=20)\
            .grid(row=0, column=2, padx=5)

        # อีเวนต์หยุดเธรด
        self.stop_event = threading.Event()

        # ---- สร้าง OverlayWindow ----
        # เริ่มต้นสร้าง OverlayWindow ขนาดและตำแหน่งจะอัปเดตใหม่ตาม crop_rect ใน translate_loop
        self.overlay = OverlayWindow(width=600, height=300, x=100, y=100)

        self.root.mainloop()

    def start_translation(self):
        self.stop_event.clear()
        threading.Thread(target=self.translate_loop, daemon=True).start()

    def stop_translation(self):
        self.stop_event.set()

    def translate_loop(self):
        global crop_rect
        # สร้าง pen เพียงครั้งเดียว แล้วเก็บไว้ใน self
        if not hasattr(self, 'green_pen'):
            self.green_pen = win32ui.CreatePen(win32con.PS_SOLID, 2, win32api.RGB(0, 255, 0))
        while not self.stop_event.is_set():
            if (crop_rect[2] > crop_rect[0]) and (crop_rect[3] > crop_rect[1]):
                # 1) จับภาพเฉพาะบริเวณที่เลือก
                img = ImageGrab.grab(bbox=tuple(crop_rect))
                img_np = np.array(img)
                # 2) OCR ด้วย pytesseract (ใช้ output_type=Output.DICT เพื่อได้ bounding box)
                ocr_language = self.selected_ocr_language.get()
                data = pytesseract.image_to_data(img_np, lang=ocr_language, output_type=Output.DICT)

                # 3) วนลูปข้อความที่ตรวจจับได้
                text_info = []
                for i in range(len(data['text'])):
                    text = data['text'][i].strip()
                    if text:
                        x = data['left'][i]
                        y = data['top'][i]
                        w = data['width'][i]
                        h = data['height'][i]
                        target_language = self.selected_language.get()
                        translated = GoogleTranslator(source='auto', target=target_language).translate(text)
                        text_info.append((x, y, w, h, translated))

                # 4) ปรับขนาดและตำแหน่ง Overlay ให้ตรงกับ crop_rect
                overlay_w = crop_rect[2] - crop_rect[0]
                overlay_h = crop_rect[3] - crop_rect[1]
                self.overlay.width = overlay_w
                self.overlay.height = overlay_h
                self.overlay.x = crop_rect[0]
                self.overlay.y = crop_rect[1]

                def draw_func(dc_temp, w, h):
                    # เปลี่ยนสีของข้อความแปลเป็นสีแดงชัดเจน
                    dc_temp.SetTextColor(win32api.RGB(0, 0, 255))
                    dc_temp.SetBkMode(win32con.TRANSPARENT)
                    old_pen = dc_temp.SelectObject(self.green_pen)
                    for (x, y, bw, bh, t) in text_info:
                        dc_temp.Rectangle((x, y, x + bw, y + bh))
                        dc_temp.TextOut(x, y - 20, t)
                    dc_temp.SelectObject(old_pen)

            self.overlay.update_overlay(draw_func)
        time.sleep(1)  # หน่วงเวลาเล็กน้อย

    # เมื่อหยุด ให้เคลียร์ overlay (หรืออัปเดตด้วยฟังก์ชันวาดว่างเปล่า)
        def clear_draw(dc_temp, w, h):
            pass
        self.overlay.update_overlay(clear_draw)
        
# ======================
# 5) เรียกโปรแกรม
# ======================
if __name__ == "__main__":
    # ตั้งค่า Tesseract หากไม่ได้อยู่ใน PATH
    pytesseract.pytesseract.tesseract_cmd = r"C:\Program Files\Tesseract-OCR\tesseract.exe"

    TranslatorApp()
