using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Drawing;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using System.Windows.Forms;
using MicroTaskPager.Core.API;
using MicroTaskPager.GUI;
using Serilog;

namespace MicroTaskPager.Core
{
    public class WindowManager
    {
        private const int MaxDesktopIndexFailures = 10;

        private static readonly ILogger _logger = Log.ForContext<WindowManager>();
        private static readonly uint _currentProcessId = (uint)Process.GetCurrentProcess().Id;
        private static readonly List<IntPtr> _hWndBlacklist = new List<IntPtr>();
        private static readonly string[] _classBlacklist = new string[]
        {
             "Windows.UI.Core.CoreWindow", // Start menu
        };

        private static readonly Dictionary<IntPtr, int> _desktopIndexFailures = new Dictionary<IntPtr, int>();

        // Icons are expensive GDI+ objects and GetOpenWindows runs several times per second,
        // so they are cached per window handle and shared between snapshots instead of recreated on every poll
        private static readonly Dictionary<IntPtr, (IntPtr Source, Bitmap Bitmap)> IconCache =
            new Dictionary<IntPtr, (IntPtr Source, Bitmap Bitmap)>();

        public static List<Window> GetOpenWindows()
        {
            var windowList = new List<Window>();
            var activeHandles = new HashSet<IntPtr>();
            var shellWindow = WinAPI.GetShellWindow();

            WinAPI.EnumWindows(
                (hWnd, lParam) =>
            {
                // Ignore specific windows
                if (hWnd == shellWindow) return true;
                if (!WinAPI.IsWindowVisible(hWnd)) return true;

                int length = WinAPI.GetWindowTextLength(hWnd);

                if (length == 0) return true;
                if (_hWndBlacklist.Contains(hWnd)) return true;

                var className = new StringBuilder(256);
                IntPtr nRet = WinAPI.GetClassName(hWnd, className, className.Capacity);
                if (_classBlacklist.Contains(className.ToString())) return true;

                WinAPI.GetWindowThreadProcessId(hWnd, out uint pid);
                if (pid == _currentProcessId) return true;

                // Get required window data
                var titleBuilder = new StringBuilder(length);
                WinAPI.GetWindowText(hWnd, titleBuilder, length + 1);

                var rect = default(WinAPI.RECT);
                WinAPI.GetWindowRect(hWnd, ref rect);

                // TODO: We can't just ignore exceptions
                // Get virtual desktop index
                int index = 0;
                try
                {
                    index = WindowsVirtualDesktopManager.GetInstance().FromDesktop(WindowsVirtualDesktopManager.GetInstance().FromWindow((IntPtr)hWnd));
                }
                catch (Exception ex)
                {
                    // Note: This is where Exception thrown: 'System.Runtime.InteropServices.COMException' comes from
                    // -> This happens when a windows gets snapped to a split view and the special window appears where the user has to select one of a list
                    //    to view for the other half of the screen
                    if (_desktopIndexFailures.TryGetValue(hWnd, out int failureCount))
                    {
                        failureCount++;
                        _desktopIndexFailures[hWnd] = failureCount;
                    }
                    else
                    {
                        _desktopIndexFailures[hWnd] = 1;
                        failureCount = 1;
                    }

                    if (failureCount >= MaxDesktopIndexFailures)
                    {
                        _logger.Debug(ex, "Window {hWnd} exceeded max desktop index failures ({count}), blacklisting", hWnd.ToString("X"), MaxDesktopIndexFailures);
                        _hWndBlacklist.Add(hWnd);
                        _desktopIndexFailures.Remove(hWnd);
                    }
                    else
                    {
                        _logger.Debug(ex, "Window {hWnd} desktop index lookup failed (attempt {count}/{max})", hWnd.ToString("X"), failureCount, MaxDesktopIndexFailures);
                    }

                    return true;
                }

                if (_desktopIndexFailures.ContainsKey(hWnd))
                {
                    _logger.Debug("Window {hWnd} recovered from desktop index failures, removing failure counter", hWnd.ToString("X"));
                    _desktopIndexFailures.Remove(hWnd);
                }

                if (index < 0) return true;

                int hIcon = WinAPI.SendMessage(hWnd, WinAPI.WM_GETICON, WinAPI.ICON_SMALL2, 0);
                if (hIcon == 0) { hIcon = WinAPI.GetClassLongPtr(hWnd, WinAPI.GCL_HICON); }
                if (hIcon == 0) { hIcon = WinAPI.LoadIcon(IntPtr.Zero, (IntPtr)WinAPI.IDI_APPLICATION); }

                activeHandles.Add(hWnd);
                windowList.Add(new Window()
                {
                    Handle = hWnd,
                    Title = titleBuilder.ToString(),
                    ProcessID = pid,
                    Class = className.ToString(),
                    ZOrder = GetWindowZOrder(hWnd),
                    Icon = GetWindowIcon(hWnd, (IntPtr)hIcon),
                    IsActive = hWnd == WinAPI.GetForegroundWindow(),
                    Dimensions = new Rectangle(rect.Left, rect.Top, rect.Right - rect.Left, rect.Bottom - rect.Top),
                    VirtualDesktopIndex = index,
                });

                return true;
            }, 0);

            PruneIconCache(activeHandles);
            PruneFailureCounters(activeHandles);

            // Process IDs would not work for windows of same process, so we use the handles now
            windowList.Sort((x, y) => x.Handle.ToInt32().CompareTo(y.Handle.ToInt32()));
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
                0, 0, 0, 0, WinAPI.SWP_NOMOVE | WinAPI.SWP_NOSIZE | WinAPI.SWP_SHOWWINDOW);
        }

        public static void RestoreWindow(IntPtr windowHandle)
        {
            //WinAPI.ShowWindowAsync(windowHandle, WinAPI.SW_RESTORE);
            WinAPI.ShowWindowAsync(windowHandle, WinAPI.SW_SHOWNOACTIVATE);
            WinAPI.SetWindowPos(windowHandle, WinAPI.HWND_TOPMOST,
                0, 0, 0, 0,
                WinAPI.SWP_NOMOVE | WinAPI.SWP_NOSIZE | WinAPI.SWP_NOACTIVATE);
        }

        private static int GetWindowZOrder(IntPtr hWnd)
        {
            var zOrder = -1;
            while ((hWnd = WinAPI.GetWindow(hWnd, WinAPI.GW_HWNDNEXT)) != IntPtr.Zero) zOrder++;
            return zOrder;
        }

        private static Bitmap GetWindowIcon(IntPtr hWnd, IntPtr hIcon)
        {
            lock (IconCache)
            {
                if (IconCache.TryGetValue(hWnd, out var cached))
                {
                    if (cached.Source == hIcon)
                    {
                        return cached.Bitmap;
                    }

                    cached.Bitmap?.Dispose();
                    IconCache.Remove(hWnd);
                }

                var bitmap = CreateWindowIcon(hIcon);
                IconCache[hWnd] = (hIcon, bitmap);
                return bitmap;
            }
        }

        private static Bitmap CreateWindowIcon(IntPtr hIcon)
        {
            if (hIcon == IntPtr.Zero)
            {
                return null;
            }

            using (var icon = Icon.FromHandle(hIcon))
            using (var source = icon.ToBitmap())
            {
                return new Bitmap(source, Dpi.Scale(16), Dpi.Scale(16));
            }
        }

        private static void PruneIconCache(HashSet<IntPtr> activeHandles)
        {
            lock (IconCache)
            {
                foreach (var handle in IconCache.Keys.Where(x => !activeHandles.Contains(x)).ToList())
                {
                    IconCache[handle].Bitmap?.Dispose();
                    IconCache.Remove(handle);
                }
            }
        }

        private static void PruneFailureCounters(HashSet<IntPtr> activeHandles)
        {
            foreach (var handle in _desktopIndexFailures.Keys.Where(x => !activeHandles.Contains(x)).ToList())
            {
                _desktopIndexFailures.Remove(handle);
            }
        }
    }
}
