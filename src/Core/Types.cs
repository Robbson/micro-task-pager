using System;
using System.Drawing;

namespace MicroTaskPager.Core
{
    public class Window
    {
        public bool IsActive { get; set; }

        public int VirtualDesktopIndex { get; set; }

        public int ZOrder { get; set; }

        public IntPtr Handle { get; set; }

        public string Title { get; set; }

        public string Class { get; set; }

        public uint ProcessID { get; set; }

        public Rectangle Dimensions { get; set; }

        public Bitmap Icon { get; set; }
    }

    // Drag and drop data has to be serializable: the WinForms drag and drop implementation
    // serializes custom data formats when the object is passed to DoDragDrop.
    // Only the handle is stored to keep the payload small and independent of GDI+ objects.
    [Serializable]
    public class DragDropData
    {
        public int OriginDesktopIndex { get; set; }

        public long DraggedWindowHandle { get; set; }
    }
}