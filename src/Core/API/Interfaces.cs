#pragma warning disable
using System;

namespace MicroTaskPager.Core.API
{
    public interface IIVirtualDesktop { }

    public interface IWindowsVirtualDesktopManager
    {
        void PinApplication(IntPtr hWnd);
        int FromDesktop(IWindowsVirtualDesktop desktop);
        IWindowsVirtualDesktop FromWindow(IntPtr hWnd);
    }

    public interface IWindowsVirtualDesktop
    {
        int Count { get; }
        IIVirtualDesktop ivd { get; set; }
        IWindowsVirtualDesktop Current { get; }
        IWindowsVirtualDesktop FromIndex(int index);

        void Restart();
        void MakeVisible();
        void MoveWindow(IntPtr hWnd);
    }
}
