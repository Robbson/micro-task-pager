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

        // High resolution icon preferred for the pager; smaller sources are only used as a fallback
        private const int IconExtractionSize = 256;

        // A newly started packaged app may need a moment before its app identity and icon are
        // available, so an unresolved fallback icon is retried periodically for a short while.
        private const int MaxIconResolutionRetries = 30;

        private static readonly TimeSpan IconResolutionRetryDelay = TimeSpan.FromSeconds(1);

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
        private static readonly Dictionary<IntPtr, IconCacheEntry> IconCache =
            new Dictionary<IntPtr, IconCacheEntry>();

        // Incremented whenever an icon is rebuilt, so the pager can repaint when a deferred icon
        // lookup (e.g. a freshly started packaged app) finally resolves.
        private static int _iconRevision;

        public static int IconRevision => _iconRevision;

        public static List<Window> GetOpenWindows(bool useSmallIcon = false)
        {
            var windowList = new List<Window>();
            var activeHandles = new HashSet<IntPtr>();
            var shellWindow = WinAPI.GetShellWindow();
            var desktopManager = WindowsVirtualDesktopManager.GetInstance();

            // Fetch the desktop list once per snapshot. Asking the shell for the
            // list for every window re-enumerates it hundreds of times per second.
            IReadOnlyList<Guid> desktopIds = null;
            int currentDesktopIndex = -1;
            try
            {
                desktopIds = desktopManager.GetDesktopIds();
                currentDesktopIndex = IndexOfDesktop(desktopIds, desktopManager.CurrentDesktopId);
            }
            catch (Exception ex)
            {
                _logger.Debug(ex, "Could not read the virtual desktop list; using per-window lookup");
            }

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
                    index = desktopIds != null
                        ? desktopManager.GetWindowDesktopIndex(hWnd, desktopIds, currentDesktopIndex)
                        : desktopManager.FromDesktop(desktopManager.FromWindow(hWnd));
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

                IntPtr hIcon = useSmallIcon
                    ? (IntPtr)WinAPI.SendMessage(hWnd, WinAPI.WM_GETICON, WinAPI.ICON_SMALL2, 0)
                    : (IntPtr)WinAPI.SendMessage(hWnd, WinAPI.WM_GETICON, WinAPI.ICON_BIG, 0);
                if (hIcon == IntPtr.Zero) { hIcon = (IntPtr)WinAPI.SendMessage(hWnd, WinAPI.WM_GETICON, WinAPI.ICON_SMALL2, 0); }
                if (hIcon == IntPtr.Zero) { hIcon = (IntPtr)WinAPI.GetClassLongPtr(hWnd, WinAPI.GCL_HICON); }

                bool useDefaultIcon = false;
                if (hIcon == IntPtr.Zero)
                {
                    hIcon = (IntPtr)WinAPI.LoadIcon(IntPtr.Zero, (IntPtr)WinAPI.IDI_APPLICATION);
                    useDefaultIcon = true;
                }

                activeHandles.Add(hWnd);
                windowList.Add(new Window()
                {
                    Handle = hWnd,
                    Title = titleBuilder.ToString(),
                    ProcessID = pid,
                    Class = className.ToString(),
                    ZOrder = GetWindowZOrder(hWnd),
                    Icon = GetWindowIcon(
                        hWnd,
                        pid,
                        hIcon,
                        useSmallIcon,
                        useDefaultIcon,
                        className.ToString() == "ApplicationFrameWindow"),
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
            ReassertTopmost(windowHandle);
        }

        // Keeps the window in the topmost band without showing, moving, resizing or activating it.
        // Reasserting on every watchdog tick is what makes partial overlaps recover even when the
        // window center is still visible.
        public static void ReassertTopmost(IntPtr windowHandle)
        {
            WinAPI.SetWindowPos(windowHandle, WinAPI.HWND_TOPMOST,
                0, 0, 0, 0,
                WinAPI.SWP_NOMOVE | WinAPI.SWP_NOSIZE | WinAPI.SWP_NOACTIVATE);
        }

        private static int IndexOfDesktop(IReadOnlyList<Guid> desktopIds, Guid id)
        {
            for (int i = 0; i < desktopIds.Count; i++)
            {
                if (desktopIds[i] == id) return i;
            }

            return -1;
        }

        private static int GetWindowZOrder(IntPtr hWnd)
        {
            var zOrder = -1;
            while ((hWnd = WinAPI.GetWindow(hWnd, WinAPI.GW_HWNDNEXT)) != IntPtr.Zero) zOrder++;
            return zOrder;
        }

        private static Bitmap GetWindowIcon(
            IntPtr hWnd, uint processId, IntPtr hIcon, bool useSmallIcon, bool useDefaultIcon, bool retryUnresolved)
        {
            lock (IconCache)
            {
                int retriesLeft = 0;
                if (IconCache.TryGetValue(hWnd, out IconCacheEntry cached))
                {
                    bool sameIcon = cached.Source == hIcon && cached.Small == useSmallIcon;
                    bool waitingForRetry = cached.RetriesLeft > 0 && DateTime.UtcNow < cached.RetryAfterUtc;
                    if (sameIcon && (cached.RetriesLeft == 0 || waitingForRetry))
                    {
                        return cached.Bitmap;
                    }

                    retriesLeft = cached.RetriesLeft;
                    cached.Bitmap?.Dispose();
                    IconCache.Remove(hWnd);
                }

                var result = CreateWindowIcon(hWnd, processId, hIcon, useSmallIcon, useDefaultIcon);
                if (result.Unresolved && retryUnresolved)
                {
                    retriesLeft = retriesLeft > 0 ? retriesLeft - 1 : MaxIconResolutionRetries;
                }
                else
                {
                    retriesLeft = 0;
                }

                IconCache[hWnd] = new IconCacheEntry
                {
                    Source = hIcon,
                    Small = useSmallIcon,
                    Bitmap = result.Bitmap,
                    RetriesLeft = retriesLeft,
                    RetryAfterUtc = DateTime.UtcNow.Add(IconResolutionRetryDelay),
                };

                // Only a resolved icon changes what is drawn, so unresolved retries do not
                // force the pager to repaint on every poll.
                if (!result.Unresolved)
                {
                    _iconRevision++;
                }

                return result.Bitmap;
            }
        }

        // The window's own icon is the one the taskbar displays, so it is preferred for every
        // size. Only when an application exposes no usable icon at all is the executable
        // inspected for a higher resolution replacement. Packaged (UWP) apps are resolved
        // through their shell app identity instead, because their icon is part of the package.
        private static (Bitmap Bitmap, bool Unresolved) CreateWindowIcon(
            IntPtr hWnd, uint processId, IntPtr hIcon, bool useSmallIcon, bool useDefaultIcon)
        {
            Bitmap windowIcon = CreateBitmapFromIcon(hIcon);

            if (useDefaultIcon)
            {
                Bitmap shellIcon = ShellIcon.TryGetWindowIcon(
                    hWnd, useSmallIcon ? Dpi.Scale(16) : IconExtractionSize, !useSmallIcon);
                if (shellIcon != null)
                {
                    windowIcon?.Dispose();
                    return (shellIcon, false);
                }
            }

            if (useSmallIcon || !useDefaultIcon)
            {
                return (windowIcon, useDefaultIcon);
            }

            Bitmap executableIcon = TryExtractExecutableIcon(processId, IconExtractionSize);
            if (executableIcon == null)
            {
                return (windowIcon, true);
            }

            if (windowIcon == null || executableIcon.Width >= windowIcon.Width)
            {
                windowIcon?.Dispose();
                return (executableIcon, false);
            }

            executableIcon.Dispose();
            return (windowIcon, false);
        }

        private static Bitmap TryExtractExecutableIcon(uint processId, int size)
        {
            try
            {
                using (var process = Process.GetProcessById((int)processId))
                {
                    string fileName = process.MainModule?.FileName;
                    if (string.IsNullOrEmpty(fileName))
                    {
                        return null;
                    }

                    var handles = new IntPtr[1];
                    uint count = WinAPI.PrivateExtractIcons(fileName, 0, size, size, handles, null, 1, 0);
                    if (count == 0 || handles[0] == IntPtr.Zero)
                    {
                        return null;
                    }

                    try
                    {
                        return CreateBitmapFromIcon(handles[0]);
                    }
                    finally
                    {
                        WinAPI.DestroyIcon(handles[0]);
                    }
                }
            }
            catch (Exception ex)
            {
                _logger.Debug(ex, "Could not extract a high resolution icon for process {ProcessId}", processId);
                return null;
            }
        }

        private static Bitmap CreateBitmapFromIcon(IntPtr hIcon)
        {
            if (hIcon == IntPtr.Zero)
            {
                return null;
            }

            using (var icon = Icon.FromHandle(hIcon))
            using (var source = icon.ToBitmap())
            {
                return new Bitmap(source);
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

        private sealed class IconCacheEntry
        {
            public IntPtr Source { get; set; }

            public bool Small { get; set; }

            public Bitmap Bitmap { get; set; }

            public int RetriesLeft { get; set; }

            public DateTime RetryAfterUtc { get; set; }
        }
    }
}
