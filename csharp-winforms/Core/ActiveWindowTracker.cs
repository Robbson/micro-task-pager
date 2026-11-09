using System;
using System.Collections.Generic;
using System.Diagnostics;
using MicroTaskPager.Core.API;
using Serilog;

namespace MicroTaskPager.Core
{
    // Remembers the last foreground window of every virtual desktop, so switching back can restore it.
    // The foreground is tracked with a WinEvent hook, otherwise a quick switch after activating a window
    // could miss the activation when only polling.
    public class ActiveWindowTracker : IDisposable
    {
        private static readonly ILogger _logger = Log.ForContext<ActiveWindowTracker>();

        private readonly Dictionary<int, IntPtr> _lastActiveWindows = new Dictionary<int, IntPtr>();
        private readonly uint _ownProcessId;

        private WinEventHook.WinEventDelegate _foregroundDelegate;
        private IntPtr _foregroundHook = IntPtr.Zero;
        private IntPtr _ignoredWindow = IntPtr.Zero;

        public ActiveWindowTracker()
        {
            using (var process = Process.GetCurrentProcess())
            {
                _ownProcessId = (uint)process.Id;
            }
        }

        public void Start(IntPtr ignoredWindow)
        {
            if (_foregroundHook != IntPtr.Zero)
            {
                return;
            }

            _ignoredWindow = ignoredWindow;
            _foregroundDelegate = OnForegroundWindowChanged;

            _foregroundHook = WinEventHook.SetWinEventHook(
                WinEventHook.EVENT_SYSTEM_FOREGROUND,
                WinEventHook.EVENT_SYSTEM_FOREGROUND,
                IntPtr.Zero,
                _foregroundDelegate,
                0,
                0,
                WinEventHook.WINEVENT_OUTOFCONTEXT | WinEventHook.WINEVENT_SKIPOWNPROCESS);

            if (_foregroundHook == IntPtr.Zero)
            {
                _logger.Warning("Could not install the foreground window hook, last active windows won't be remembered");
            }
        }

        // Tracks a window as the last active one on its virtual desktop
        public void RememberWindow(IntPtr hwnd)
        {
            if (hwnd == IntPtr.Zero || hwnd == _ignoredWindow)
            {
                return;
            }

            WinAPI.GetWindowThreadProcessId(hwnd, out uint processId);
            if (processId == _ownProcessId)
            {
                return;
            }

            // Only remember real application windows, the same ones the pager is able to show
            if (WinAPI.GetWindowTextLength(hwnd) == 0)
            {
                return;
            }

            try
            {
                var manager = WindowsVirtualDesktopManager.GetInstance();
                int desktopIndex = manager.FromDesktop(manager.FromWindow(hwnd));
                if (desktopIndex >= 0)
                {
                    _lastActiveWindows[desktopIndex] = hwnd;
                }
            }
            catch (Exception ex)
            {
                _logger.Debug(ex, "Could not remember the active window {Handle}", hwnd);
            }
        }

        // Brings back the last active window of a desktop, if it still exists on that desktop
        public void RestoreLastActiveWindow(int desktopIndex)
        {
            if (!_lastActiveWindows.TryGetValue(desktopIndex, out IntPtr hwnd))
            {
                return;
            }

            if (hwnd == IntPtr.Zero || !WinAPI.IsWindow(hwnd) || !WinAPI.IsWindowVisible(hwnd))
            {
                _lastActiveWindows.Remove(desktopIndex);
                return;
            }

            try
            {
                var manager = WindowsVirtualDesktopManager.GetInstance();
                if (manager.FromDesktop(manager.FromWindow(hwnd)) != desktopIndex)
                {
                    return;
                }
            }
            catch (Exception ex)
            {
                _logger.Debug(ex, "Could not validate the last active window of desktop {DesktopIndex}", desktopIndex);
                return;
            }

            if (WinAPI.IsIconic(hwnd))
            {
                WinAPI.ShowWindow(hwnd, WinAPI.SW_RESTORE);
            }

            if (!WinAPI.SetForegroundWindow(hwnd))
            {
                _logger.Debug("Could not restore the last active window {Handle} of desktop {DesktopIndex}", hwnd, desktopIndex);
            }
        }

        public void Dispose()
        {
            if (_foregroundHook != IntPtr.Zero)
            {
                WinEventHook.UnhookWinEvent(_foregroundHook);
                _foregroundHook = IntPtr.Zero;
            }

            _foregroundDelegate = null;
            GC.SuppressFinalize(this);
        }

        private void OnForegroundWindowChanged(
            IntPtr hWinEventHook,
            uint eventType,
            IntPtr hwnd,
            int idObject,
            int idChild,
            uint dwEventThread,
            uint dwmsEventTime)
        {
            if (eventType != WinEventHook.EVENT_SYSTEM_FOREGROUND)
            {
                return;
            }

            RememberWindow(hwnd);
        }
    }
}
