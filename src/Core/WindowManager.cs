using MicroTaskPager.Core.API;
using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Drawing;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using System.Windows.Forms;

namespace MicroTaskPager
{
    public class WindowManager
    {
        static readonly List<IntPtr> hWndBlacklist = new List<IntPtr>();
        static readonly string[] classBlacklist = new string[] {
             "Windows.UI.Core.CoreWindow" // Start menu
        };

        public static List<Window> GetOpenWindows()
        {
            var windowList = new List<Window>();
            var shellWindow = WinAPI.GetShellWindow();

            WinAPI.EnumWindows((IntPtr hWnd, int lParam) =>
            {
                // Ignore specific windows
                if (hWnd == shellWindow) return true;
                if (!WinAPI.IsWindowVisible(hWnd)) return true;

                int length = WinAPI.GetWindowTextLength(hWnd);

                if (length == 0) return true;
                if (hWndBlacklist.Contains(hWnd)) return true;

                var className = new StringBuilder(256);
                IntPtr nRet = WinAPI.GetClassName(hWnd, className, className.Capacity);
                if (classBlacklist.Contains(className.ToString())) return true;

                // Get required window data
                var titleBuilder = new StringBuilder(length);
                WinAPI.GetWindowText(hWnd, titleBuilder, length + 1);

                var rect = new WinAPI.RECT();
                WinAPI.GetWindowRect(hWnd, ref rect);

                // Get virtual desktop index
                int index = 0;
                WinAPI.GetWindowThreadProcessId(hWnd, out uint pid);
                try { index = WindowsVirtualDesktopManager.GetInstance().FromDesktop(WindowsVirtualDesktopManager.GetInstance().FromWindow((IntPtr)hWnd)); }
                catch
                {
                    // Note: This is where Exception thrown: 'System.Runtime.InteropServices.COMException' comes from
                    // All windows where this happens are getting blacklisted
                    hWndBlacklist.Add(hWnd);
                    return true;
                }
                if (index < 0) return true;

                int hIcon = WinAPI.SendMessage(hWnd, WinAPI.WM_GETICON, WinAPI.ICON_SMALL2, 0);
                if (hIcon == 0) { hIcon = WinAPI.GetClassLongPtr(hWnd, WinAPI.GCL_HICON); }
                if (hIcon == 0) { hIcon = WinAPI.LoadIcon(IntPtr.Zero, (IntPtr)WinAPI.IDI_APPLICATION); }

                windowList.Add(new Window()
                {
                    Handle = hWnd,
                    Title = titleBuilder.ToString(),
                    ProcessID = pid,
                    Class = className.ToString(),
                    ZOrder = GetWindowZOrder(hWnd),
                    Icon = hIcon != 0 ? new Bitmap(Icon.FromHandle((IntPtr)hIcon).ToBitmap(), 16, 16) : null,
                    IsActive = hWnd == WinAPI.GetForegroundWindow(),
                    Dimensions = new Rectangle(rect.Left, rect.Top, rect.Right - rect.Left, rect.Bottom - rect.Top),
                    VirtualDesktopIndex = index
                });

                return true;
            }, 0);

            windowList.Sort((x, y) => x.ProcessID.CompareTo(y.ProcessID));
            return windowList;
        }

        public static Window GetActiveWindow()
        {
            var hwnd = WinAPI.GetForegroundWindow();
            return GetOpenWindows().SingleOrDefault(x => x.Handle == hwnd);
        }

        public static void SetAlwaysOnTop(IntPtr handle, bool value)
        {
            WinAPI.SetWindowPos(handle, value ? WinAPI.HWND_TOPMOST : WinAPI.HWND_NOTOPMOST,
                0, 0, 0, 0, WinAPI.SWP_NOMOVE | WinAPI.SWP_NOSIZE | WinAPI.SWP_SHOWWINDOW
            );
        }

        public static void RestoreWindow(IntPtr windowHandle)
        {
            //WinAPI.ShowWindowAsync(windowHandle, WinAPI.SW_RESTORE);
            WinAPI.ShowWindowAsync(windowHandle, WinAPI.SW_SHOWNOACTIVATE);
            WinAPI.SetWindowPos(windowHandle, WinAPI.HWND_TOPMOST,
                0, 0, 0, 0,
                WinAPI.SWP_NOMOVE | WinAPI.SWP_NOSIZE | WinAPI.SWP_NOACTIVATE);
        }

        static int GetWindowZOrder(IntPtr hWnd)
        {
            var zOrder = -1;
            while ((hWnd = WinAPI.GetWindow(hWnd, WinAPI.GW_HWNDNEXT)) != IntPtr.Zero) zOrder++;
            return zOrder;
        }
    }
}
