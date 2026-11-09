#pragma warning disable
using System;
using System.Collections.Generic;

namespace MicroTaskPager.Core.API
{
    public interface IIVirtualDesktop { }

    public interface IWindowsVirtualDesktopManager
    {
        void PinApplication(IntPtr hWnd);
        int FromDesktop(IWindowsVirtualDesktop desktop);
        IWindowsVirtualDesktop FromWindow(IntPtr hWnd);
        IReadOnlyList<Guid> GetDesktopIds();
        Guid CurrentDesktopId { get; }
        int GetWindowDesktopIndex(IntPtr hWnd, IReadOnlyList<Guid> desktopIds, int currentDesktopIndex);
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
