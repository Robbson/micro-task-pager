using System;
using System.Drawing;
using System.Runtime.InteropServices;
using System.Text;

namespace MicroTaskPager.Core.API
{
    // Resolves the shell icon of a window's application. This is mainly needed for packaged
    // (UWP) apps, whose top-level window is hosted by ApplicationFrameHost and does not expose
    // the app's icon through WM_GETICON or through the process executable.
    public static class ShellIcon
    {
        private const uint VariantTypeLpwstr = 31;
        private const int SiigbfBiggerSizeOk = 0x01;
        private const int SiigbfIconOnly = 0x04;

        private static readonly PropertyKey AppUserModelIdKey = new PropertyKey
        {
            FormatId = new Guid("9F4C2855-9F79-4B39-A8D0-E1D42DE1D5F3"),
            PropertyId = 5,
        };

        [ComImport]
        [Guid("886D8EEB-8CF2-4446-8D02-CDBA1DBDCF99")]
        [InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
        private interface IPropertyStore
        {
            [PreserveSig]
            int GetCount(out uint propertyCount);

            [PreserveSig]
            int GetAt(uint propertyIndex, out PropertyKey key);

            [PreserveSig]
            int GetValue(ref PropertyKey key, out PropVariant value);

            [PreserveSig]
            int SetValue(ref PropertyKey key, ref PropVariant value);

            [PreserveSig]
            int Commit();
        }

        [ComImport]
        [Guid("BCC18B79-BA16-442F-80C4-8A59C30C463B")]
        [InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
        private interface IShellItemImageFactory
        {
            [PreserveSig]
            int GetImage(ShellSize size, int flags, out IntPtr hBitmap);
        }

        public static Bitmap TryGetWindowIcon(IntPtr hWnd, int size, bool allowLarger = true)
        {
            string appUserModelId = TryGetAppUserModelId(hWnd);
            if (string.IsNullOrEmpty(appUserModelId))
            {
                return null;
            }

            return TryGetIconFromAppsFolder(appUserModelId, size, allowLarger);
        }

        private static string TryGetAppUserModelId(IntPtr hWnd)
        {
            string appUserModelId = TryGetWindowAppUserModelId(hWnd);
            if (!string.IsNullOrEmpty(appUserModelId))
            {
                return appUserModelId;
            }

            // Packaged apps are hosted by ApplicationFrameHost. Their top-level window does not
            // carry the app identity, but the child window belongs to the real app process.
            var className = new StringBuilder(256);
            WinAPI.GetClassName(hWnd, className, className.Capacity);
            if (className.ToString() != "ApplicationFrameWindow")
            {
                return null;
            }

            return TryGetHostedAppUserModelId(hWnd);
        }

        private static string TryGetWindowAppUserModelId(IntPtr hWnd)
        {
            IPropertyStore propertyStore = null;
            try
            {
                Guid iid = typeof(IPropertyStore).GUID;
                if (SHGetPropertyStoreForWindow(hWnd, ref iid, out propertyStore) != 0 || propertyStore == null)
                {
                    return null;
                }

                PropertyKey key = AppUserModelIdKey;
                if (propertyStore.GetValue(ref key, out PropVariant value) != 0)
                {
                    return null;
                }

                try
                {
                    if (value.VariantType != VariantTypeLpwstr || value.PointerValue == IntPtr.Zero)
                    {
                        return null;
                    }

                    return Marshal.PtrToStringUni(value.PointerValue);
                }
                finally
                {
                    PropVariantClear(ref value);
                }
            }
            catch
            {
                return null;
            }
            finally
            {
                if (propertyStore != null)
                {
                    Marshal.ReleaseComObject(propertyStore);
                }
            }
        }

        private static string TryGetHostedAppUserModelId(IntPtr hWnd)
        {
            WinAPI.GetWindowThreadProcessId(hWnd, out uint hostProcessId);
            string coreWindowAppUserModelId = null;
            string fallbackAppUserModelId = null;

            try
            {
                WinAPI.EnumChildWindows(hWnd, (childWindow, lParam) =>
                {
                    WinAPI.GetWindowThreadProcessId(childWindow, out uint processId);
                    if (processId == 0 || processId == hostProcessId)
                    {
                        return true;
                    }

                    string appUserModelId = TryGetProcessAppUserModelId(processId);
                    if (string.IsNullOrEmpty(appUserModelId))
                    {
                        return true;
                    }

                    var className = new StringBuilder(256);
                    WinAPI.GetClassName(childWindow, className, className.Capacity);
                    if (className.ToString() == "Windows.UI.Core.CoreWindow")
                    {
                        coreWindowAppUserModelId = appUserModelId;
                        return false;
                    }

                    if (fallbackAppUserModelId == null)
                    {
                        fallbackAppUserModelId = appUserModelId;
                    }

                    return true;
                }, 0);
            }
            catch
            {
                return null;
            }

            return coreWindowAppUserModelId ?? fallbackAppUserModelId;
        }

        private static string TryGetProcessAppUserModelId(uint processId)
        {
            try
            {
                using (var process = System.Diagnostics.Process.GetProcessById((int)processId))
                {
                    uint length = 256;
                    var builder = new StringBuilder((int)length);
                    int result = GetApplicationUserModelId(process.Handle, ref length, builder);
                    if (result != 0)
                    {
                        builder = new StringBuilder((int)length);
                        result = GetApplicationUserModelId(process.Handle, ref length, builder);
                    }

                    return result == 0 ? builder.ToString() : null;
                }
            }
            catch
            {
                return null;
            }
        }

        private static Bitmap TryGetIconFromAppsFolder(string appUserModelId, int size, bool allowLarger)
        {
            object shellItem = null;
            try
            {
                Guid iid = typeof(IShellItemImageFactory).GUID;
                string parsingName = "shell:AppsFolder\\" + appUserModelId;
                if (SHCreateItemFromParsingName(parsingName, IntPtr.Zero, ref iid, out shellItem) != 0
                    || !(shellItem is IShellItemImageFactory imageFactory))
                {
                    return null;
                }

                var shellSize = new ShellSize(size, size);
                int flags = SiigbfIconOnly | (allowLarger ? SiigbfBiggerSizeOk : 0);
                if (imageFactory.GetImage(shellSize, flags, out IntPtr hBitmap) != 0
                    || hBitmap == IntPtr.Zero)
                {
                    return null;
                }

                try
                {
                    return CreateBitmapPreservingAlpha(hBitmap);
                }
                finally
                {
                    WinAPI.DeleteObject(hBitmap);
                }
            }
            catch
            {
                return null;
            }
            finally
            {
                if (shellItem != null)
                {
                    Marshal.ReleaseComObject(shellItem);
                }
            }
        }

        // Image.FromHbitmap drops the alpha channel, which turns the transparent corners of an
        // app icon into black. Reading the DIB bits directly keeps the 32-bit alpha data.
        private static Bitmap CreateBitmapPreservingAlpha(IntPtr hBitmap)
        {
            NativeBitmap native = default;
            if (GetObject(hBitmap, Marshal.SizeOf(typeof(NativeBitmap)), ref native) == 0)
            {
                return null;
            }

            int width = native.Width;
            int height = Math.Abs(native.Height);
            if (width <= 0 || height <= 0)
            {
                return null;
            }

            var bitmapInfo = new BitmapInfo
            {
                Header = new BitmapInfoHeader
                {
                    Size = (uint)Marshal.SizeOf(typeof(BitmapInfoHeader)),
                    Width = width,
                    Height = -height,
                    Planes = 1,
                    BitCount = 32,
                    Compression = 0,
                },
            };

            int stride = width * 4;
            var pixels = new byte[stride * height];
            IntPtr hdc = GetDC(IntPtr.Zero);
            if (hdc == IntPtr.Zero)
            {
                return null;
            }

            try
            {
                IntPtr buffer = Marshal.AllocHGlobal(pixels.Length);
                try
                {
                    int lines = GetDIBits(hdc, hBitmap, 0, (uint)height, buffer, ref bitmapInfo, 0);
                    if (lines == 0)
                    {
                        return null;
                    }

                    Marshal.Copy(buffer, pixels, 0, pixels.Length);
                }
                finally
                {
                    Marshal.FreeHGlobal(buffer);
                }
            }
            finally
            {
                ReleaseDC(IntPtr.Zero, hdc);
            }

            var result = new Bitmap(width, height, System.Drawing.Imaging.PixelFormat.Format32bppArgb);
            var data = result.LockBits(
                new Rectangle(0, 0, width, height),
                System.Drawing.Imaging.ImageLockMode.WriteOnly,
                System.Drawing.Imaging.PixelFormat.Format32bppArgb);
            try
            {
                Marshal.Copy(pixels, 0, data.Scan0, pixels.Length);
            }
            finally
            {
                result.UnlockBits(data);
            }

            return result;
        }

        [DllImport("shell32.dll")]
        private static extern int SHGetPropertyStoreForWindow(
            IntPtr hWnd, ref Guid iid, [MarshalAs(UnmanagedType.Interface)] out IPropertyStore propertyStore);

        [DllImport("shell32.dll", CharSet = CharSet.Unicode)]
        private static extern int SHCreateItemFromParsingName(
            string path, IntPtr bindContext, ref Guid iid, [MarshalAs(UnmanagedType.Interface)] out object shellItem);

        [DllImport("ole32.dll")]
        private static extern int PropVariantClear(ref PropVariant value);

        [DllImport("kernel32.dll", CharSet = CharSet.Unicode)]
        private static extern int GetApplicationUserModelId(
            IntPtr hProcess, ref uint applicationUserModelIdLength, StringBuilder applicationUserModelId);

        [DllImport("user32.dll")]
        private static extern IntPtr GetDC(IntPtr hWnd);

        [DllImport("user32.dll")]
        private static extern int ReleaseDC(IntPtr hWnd, IntPtr hDC);

        [DllImport("gdi32.dll")]
        private static extern int GetObject(IntPtr hObject, int bufferSize, ref NativeBitmap bitmap);

        [DllImport("gdi32.dll")]
        private static extern int GetDIBits(
            IntPtr hDC, IntPtr hBitmap, uint startScan, uint scanLines, IntPtr bits, ref BitmapInfo bitmapInfo, uint usage);

        [StructLayout(LayoutKind.Sequential)]
        private struct PropertyKey
        {
            public Guid FormatId;
            public uint PropertyId;
        }

        [StructLayout(LayoutKind.Explicit)]
        private struct PropVariant
        {
            [FieldOffset(0)]
            public ushort VariantType;

            [FieldOffset(8)]
            public IntPtr PointerValue;
        }

        [StructLayout(LayoutKind.Sequential)]
        private struct ShellSize
        {
            public int Width;
            public int Height;

            public ShellSize(int width, int height)
            {
                Width = width;
                Height = height;
            }
        }

        [StructLayout(LayoutKind.Sequential)]
        private struct NativeBitmap
        {
            public int Type;
            public int Width;
            public int Height;
            public int WidthBytes;
            public ushort Planes;
            public ushort BitsPixel;
            public IntPtr Bits;
        }

        [StructLayout(LayoutKind.Sequential)]
        private struct BitmapInfoHeader
        {
            public uint Size;
            public int Width;
            public int Height;
            public ushort Planes;
            public ushort BitCount;
            public uint Compression;
            public uint SizeImage;
            public int XPelsPerMeter;
            public int YPelsPerMeter;
            public uint ColorsUsed;
            public uint ColorsImportant;
        }

        [StructLayout(LayoutKind.Sequential)]
        private struct BitmapInfo
        {
            public BitmapInfoHeader Header;
            public uint Colors;
        }
    }
}
