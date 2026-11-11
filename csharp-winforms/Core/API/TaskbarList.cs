using System;
using System.Runtime.InteropServices;

namespace MicroTaskPager.Core.API
{
    [ComImport]
    [InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    [Guid("56FDF342-FD6D-11d0-958A-006097C9A090")]
    public interface ITaskbarList
    {
        void HrInit();

        void AddTab(IntPtr hwnd);

        void DeleteTab(IntPtr hwnd);

        void ActivateTab(IntPtr hwnd);

        void SetActiveAlt(IntPtr hwnd);
    }

    // Shell API used to add or remove a window's taskbar button without changing the window's
    // extended styles. Removing the button with ITaskbarList keeps the window's application view,
    // so virtual-desktop pinning still works while the app is hidden from the taskbar.
    public static class TaskbarList
    {
        private static readonly Guid Clsid = new Guid("56FDF344-FD6D-11d0-958A-006097C9A090");

        public static ITaskbarList Create()
        {
            var instance = (ITaskbarList)Activator.CreateInstance(Type.GetTypeFromCLSID(Clsid));
            instance.HrInit();
            return instance;
        }
    }
}
