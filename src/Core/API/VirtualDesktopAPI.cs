#pragma warning disable
using System;

namespace MicroTaskPager.Core.API
{
    public class WindowsVirtualDesktop
    {
        private static IWindowsVirtualDesktop _instance;
        public static void Restart()
        {
            WindowsVirtualDesktop._instance = null;
            GetInstance();
            WindowsVirtualDesktop._instance.Restart();
        }

        public static IWindowsVirtualDesktop GetInstance()
        {
            if (WindowsVirtualDesktop._instance == null)
            {
                if (Program.WindowsVersion.IsWin11_22H2_OrLater())
                    _instance = new API.Win11_.WindowsVirtualDesktop();
                else if (Program.WindowsVersion.IsWin11_21H2_OrLater())
                    _instance = new API.Win11.WindowsVirtualDesktop();
                else if (Program.WindowsVersion.IsWin10())
                    _instance = new API.Win10.WindowsVirtualDesktop();
                else if (Program.WindowsVersion.IsWin10LTSC())
                    _instance = new API.Win10LTSC.WindowsVirtualDesktop();
                else
                    throw new PlatformNotSupportedException();
            }

            return WindowsVirtualDesktop._instance;
        }
    }

    public class WindowsVirtualDesktopManager
    {
        private static IWindowsVirtualDesktopManager _instance;
        public static void Restart() => WindowsVirtualDesktopManager._instance = null;

        public static IWindowsVirtualDesktopManager GetInstance()
        {
            if (WindowsVirtualDesktopManager._instance == null)
            {
                if (Program.WindowsVersion.IsWin11_22H2_OrLater())
                    _instance = new API.Win11_.WindowsVirtualDesktopManager();
                else if (Program.WindowsVersion.IsWin11_21H2_OrLater())
                    _instance = new API.Win11.WindowsVirtualDesktopManager();
                else if (Program.WindowsVersion.IsWin10())
                    _instance = new API.Win10.WindowsVirtualDesktopManager();
                else if (Program.WindowsVersion.IsWin10LTSC())
                    _instance = new API.Win10LTSC.WindowsVirtualDesktopManager();
                else
                    throw new PlatformNotSupportedException();
            }

            return WindowsVirtualDesktopManager._instance;
        }
    }
}
