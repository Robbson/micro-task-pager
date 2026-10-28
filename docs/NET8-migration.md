# .NET 8 Migration Notes

This document describes the runtime-compatibility fixes that were necessary after moving
Micro Task Pager from .NET Framework 4.8 to .NET 8. The project compiled and started after the
upgrade, but several Windows interop and WinForms behaviors changed in ways that only appeared at
runtime.

## Symptom summary

| Symptom | Root cause | Main fix |
| --- | --- | --- |
| Pager no longer stays visible when switching virtual desktops | `.NET 8` cannot marshal COM interfaces declared as `ComInterfaceType.InterfaceIsIInspectable`; automatic pinning failed on every update | Declare `IApplicationView` as `InterfaceIsIUnknown` with the three `IInspectable` methods added manually |
| Dragging window icons between desktops no longer moves windows | Same `IInspectable` failure in `GetViewForHwnd`/`MoveViewToDesktop`, plus `DragDropData` was not serializable for the .NET 8 WinForms `DataObject` | Fix the COM declaration and make the drag payload serializable and primitive-only |
| Pinned application windows (especially the Settings window) were handled differently than on .NET Framework | Pinned windows return the special `AppOnAllDesktops`/`WindowOnAllDesktops` GUIDs, which are not part of the normal desktop list. `FromWindow` returned `null` and the window was skipped | Map the special GUIDs to the current desktop |
| Settings/About dialogs should not appear as pager icons | On .NET Framework this happened only accidentally because pinned-window GUIDs were not mapped. After fixing pinning, those windows became visible | Explicitly filter out all windows belonging to the pager process |
| Windows could disappear permanently after a transient COM error | `WindowManager` added a window to its permanent blacklist on the first `COMException` | Retry a window several times before blacklisting it, and prune stale failure entries |

## Background: what changed in .NET

### 1. Built-in WinRT/`IInspectable` support was removed

.NET Framework contained built-in support for Windows Runtime (WinRT) interop. That support was
removed from .NET Core and does not exist in .NET 5+. Windows Forms applications that use
undocumented virtual-desktop COM interfaces are affected because several of those interfaces are
WinRT-style `IInspectable` interfaces.

The application log contained the following exception while trying to pin the pager:

```text
Marshalling as IInspectable is not supported in the .NET runtime.
```

The exception came from `IApplicationViewCollection.GetViewForHwnd`, which returns an
`IApplicationView`. `IApplicationView` was declared like this:

```csharp
[ComImport]
[InterfaceType(ComInterfaceType.InterfaceIsIInspectable)]
[Guid("372E1D3B-38D3-42E4-A15B-8AB2B178F513")]
public interface IApplicationView
{
    int SetFocus();
    // ...
}
```

`ComInterfaceType.InterfaceIsIInspectable` is not supported by the .NET 8 COM marshaller, so every
call through that interface failed.

### 2. WinForms drag-and-drop requires serializable custom data

.NET Framework's drag-and-drop implementation could return a non-serializable managed object from
`IDataObject.GetData` when the drag stayed inside the same process. In modern .NET (the breaking
change was introduced in .NET 7), custom data passed to `Control.DoDragDrop` is serialized through
the OLE data object. If the custom type is not serializable, `GetData` returns `null`.

The old payload contained a `Window` object, including a `Bitmap`:

```csharp
public class DragDropData
{
    public int OriginDesktopIndex { get; set; }
    public Window DraggedWindow { get; set; }
}
```

That was both serialization-hostile and unnecessarily large for a simple same-process drag.

### 3. Pinned windows use special virtual-desktop GUIDs

`IVirtualDesktopManager.GetWindowDesktopId` returns one of two synthetic GUIDs for windows that are
pinned to all desktops:

```text
AppOnAllDesktops:    BB64D5B7-4DE3-4AB2-A87C-DB7601AEA7DC
WindowOnAllDesktops: C2DDEA68-66F2-4CF9-8264-1BFD00FBBBAC
```

Those GUIDs are not returned by the normal virtual-desktop list. The application's `FromWindow`
implementation searched the desktop list for the ID and returned `null` when no match was found.
`WindowManager.GetOpenWindows` skipped such windows.

Before pinning was fixed, the pager often was not pinned automatically, so this usually did not
matter. Once automatic pinning worked, the pager and its Settings window were both part of a pinned
app and therefore reported the special GUID.

## Fix 1: Declare `IApplicationView` as `IUnknown` plus explicit `IInspectable` methods

An `IInspectable` interface has the same three base methods as `IInspectable` itself before its own
methods:

```text
IUnknown:      QueryInterface, AddRef, Release
IInspectable:  GetIids, GetRuntimeClassName, GetTrustLevel
IApplicationView: SetFocus, SwitchTo, ...
```

The .NET COM marshaller can handle a plain `InterfaceIsIUnknown` interface, so the three
`IInspectable` methods are declared explicitly. This preserves the vtable layout while avoiding the
unsupported `InterfaceIsIInspectable` marshalling mode.

```csharp
[ComImport]
[InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
[Guid("372E1D3B-38D3-42E4-A15B-8AB2B178F513")]
public interface IApplicationView
{
    int GetIids(out int iidCount, out IntPtr iids);
    int GetRuntimeClassName(out IntPtr className);
    int GetTrustLevel(out int trustLevel);

    int SetFocus();
    int SwitchTo();
    // ...
}
```

This change was applied to every version-specific API implementation:

- `src/Core/API/VirtualDesktopAPI-Win10.cs`
- `src/Core/API/VirtualDesktopAPI-Win10LTSC.cs`
- `src/Core/API/VirtualDesktopAPI-Win11.cs`
- `src/Core/API/VirtualDesktopAPI-Win11_.cs` (Windows 11 24H2/25H2 layout)

Keeping all four files in sync is important: the project selects the implementation at runtime based
on the detected Windows version.

### Why this restored pinning

`PinApplication` uses `GetAppId`, which calls `GetViewForHwnd` and then
`IApplicationView.GetAppUserModelId`. With the old declaration, `GetViewForHwnd` threw
`PlatformNotSupportedException` before pinning could happen. With the fixed
declaration, the app ID can be read and `IVirtualDesktopPinnedApps.PinAppID` succeeds.

### Why this restored window moves

`MoveWindow` also uses `GetViewForHwnd` and then
`IVirtualDesktopManagerInternal.MoveViewToDesktop`. The same interop failure was why drag-and-drop
between desktops did not move windows even when the drag itself started.

## Fix 2: Make the drag-and-drop payload serializable and small

`DragDropData` was changed to a serializable type that contains only the desktop index and the
window handle:

```csharp
[Serializable]
public class DragDropData
{
    public int OriginDesktopIndex { get; set; }

    public long DraggedWindowHandle { get; set; }
}
```

The handle is stored as `long` rather than `IntPtr` to keep the payload simple and clearly
serializable. The source desktop writes the handle in `OnMouseDown`:

```csharp
_dragDropData = new DragDropData
{
    OriginDesktopIndex = VirtualDesktopIndex,
    DraggedWindowHandle = w.Handle.ToInt64(),
};
```

The drop target reads it back, checks for `null` (the .NET 8 behavior when data cannot be
materialized), and converts the value back to `IntPtr`:

```csharp
var ddd = e.Data.GetData(typeof(DragDropData)) as DragDropData;
if (ddd != null && e.Effect == DragDropEffects.Move)
{
    WindowsVirtualDesktop.GetInstance()
        .FromIndex(VirtualDesktopIndex)
        .MoveWindow(new IntPtr(ddd.DraggedWindowHandle));
}
```

This avoids serializing GDI+ objects such as `Bitmap` and removes the dependency on the old
`Window` object graph.

> Future note: .NET 9 and later remove `BinaryFormatter`-based serialization more aggressively. If
> the project is upgraded again, either implement a custom `IDataObject` for the in-process drag or
> keep the drag payload in a process-local field instead of relying on `DataObject` serialization.

## Fix 3: Map pinned-window GUIDs to the current desktop

`FromWindow` now recognizes the two special GUIDs and returns the current desktop:

```csharp
Guid id = VirtualDesktopManagerPrivate.GetWindowDesktopId(hWnd);
if (id == WindowsVirtualDesktopAPI.APP_ON_ALL_DESKTOPS ||
    id == WindowsVirtualDesktopAPI.WINDOW_ON_ALL_DESKTOPS)
{
    return new WindowsVirtualDesktop(VirtualDesktopManagerInternal.GetCurrentDesktop());
}
```

The constants live in each `WindowsVirtualDesktopAPI` class:

```csharp
public static readonly Guid APP_ON_ALL_DESKTOPS =
    new Guid("BB64D5B7-4DE3-4AB2-A87C-DB7601AEA7DC");

public static readonly Guid WINDOW_ON_ALL_DESKTOPS =
    new Guid("C2DDEA68-66F2-4CF9-8264-1BFD00FBBBAC");
```

This mapping was added to all four API implementations.

A pinned window is visible on every virtual desktop, so representing it on the currently active
desktop is the least surprising behavior for the pager. It also prevents pinned windows from being
dropped from the pager just because their desktop ID is synthetic.

## Fix 4: Explicitly exclude the pager's own windows

The Settings window is supposed to be invisible in the pager. On .NET Framework this was true only
by accident: the pager was often pinned, the Settings window inherited the pinned app's
`AppOnAllDesktops` GUID, and `FromWindow` could not map that GUID to a desktop index.

Once Fix 3 made pinned windows visible, the Settings window started appearing in the pager. The
correct behavior is now enforced explicitly in `WindowManager.GetOpenWindows`:

```csharp
private static readonly uint _currentProcessId =
    (uint)Process.GetCurrentProcess().Id;

// ...

WinAPI.GetWindowThreadProcessId(hWnd, out uint pid);
if (pid == _currentProcessId) return true;
```

This excludes:

- the WPF Settings window,
- the About dialog,
- WPF-internal helper windows,
- any future dialog owned by the pager process.

The pager's main window itself was already excluded because it has no window title. The
process-ID filter makes the rule independent of titles, classes, and pin state.

## Fix 5: Do not blacklist a window on its first transient COM error

`FromWindow` can throw `COMException` with `TYPE_E_ELEMENTNOTFOUND` in two situations:

- a newly created window is not yet registered in the virtual desktop application-view collection;
- Windows creates a special window for snap assist/split view that is not a normal application view.

The previous implementation added the handle to `_hWndBlacklist` on the first exception. That made
the window disappear from the pager for the rest of the process lifetime, even if the error was only
temporary.

`WindowManager` now tracks consecutive failures per handle:

```csharp
private const int MaxDesktopIndexFailures = 10;
private static readonly Dictionary<IntPtr, int> _desktopIndexFailures =
    new Dictionary<IntPtr, int>();
```

Behavior:

- A failed lookup is retried on the next update and logged at `Debug` level.
- The handle is blacklisted only after `MaxDesktopIndexFailures` consecutive failures.
- A successful lookup removes the failure counter.
- Counters for destroyed windows are pruned from the dictionary.

With the default secondary update interval, ten failures correspond to roughly five seconds. That
is long enough for normal window registration to complete, while still preventing a permanently
invalid window from being queried forever.

## Verification performed

- `dotnet build src/MicroTaskPager.csproj -c Debug` completes with 0 warnings and 0 errors.
- The pager pins itself successfully after startup; the virtual-desktop COM query reports the app as
  pinned.
- Switching virtual desktops keeps the pager visible.
- Dragging a window icon between desktop panels moves the window.
- The Settings window no longer appears in the pager; the About dialog is excluded by the same rule.
- A small .NET 8 WinForms harness verified that a `DataObject` created from the new
  `DragDropData` type returns the original values from `GetData`.

## Release packaging changes

The .NET 8 release is published framework-dependent, because the application assumes that the
target machine has the .NET 8 Desktop Runtime installed. `src/Publish.cmd` was updated accordingly:

- It removes previous build artifacts from `prod\` before publishing, while keeping existing
  release ZIP archives.
- It publishes with `--self-contained false`, so no .NET runtime files are bundled.
- It includes `MicroTaskPager.deps.json` and `MicroTaskPager.runtimeconfig.json` in the ZIP. These
  files are required by the .NET host and did not exist in the .NET Framework release.
- It no longer includes `MicroTaskPager.exe.config`. That file was a .NET Framework startup and
  binding-redirect configuration and is not used by the .NET 8 host.
- It still removes PDB files before creating the archive.

The old `prod\` folder contained several stale `System.*.dll` files from earlier .NET Framework and
package-based publishes. A clean .NET 8 publish contains only the application files, the required
JSON files, and the Serilog assemblies:

```text
MicroTaskPager.exe
MicroTaskPager.dll
MicroTaskPager.deps.json
MicroTaskPager.runtimeconfig.json
Serilog.dll
Serilog.Sinks.Async.dll
Serilog.Sinks.Debug.dll
Serilog.Sinks.File.dll
```

The `System.*` assemblies are provided by the .NET 8 shared framework and must not be shipped with a
framework-dependent application.

`README.md` was updated to require the .NET 8 Desktop Runtime, to mention the two JSON files, and to
describe the framework-dependent release package. `THIRD-PARTY-NOTICES.md` was updated as well: it
now names the .NET 8 Desktop Runtime as a prerequisite and no longer lists the `System.*` assemblies,
because those are supplied by the shared framework and are not bundled.

## Practical notes for future maintenance

1. **Keep the four API variants synchronized.** A change to `IApplicationView`,
   `IApplicationViewCollection`, or the pinned GUID handling must be applied to Windows 10,
   Windows 10 LTSC, Windows 11, and Windows 11 24H2+ implementations.
2. **Treat virtual-desktop COM interfaces as version-sensitive.** They are undocumented and their
   vtable layout can change between Windows releases. Test on each supported Windows version.
3. **Revisit drag-and-drop on the next .NET upgrade.** The current fix depends on
   `[Serializable]` custom data being supported by WinForms. A custom `IDataObject` or a
   process-local field is the safer long-term design.
4. **For new WinRT interfaces, prefer CsWinRT.** If more `IInspectable` interfaces are needed,
   using `Microsoft.Windows.CsWinRT` and its `ComWrappers` support avoids hand-maintaining
   `IInspectable` vtable stubs. The hand-written declarations above were chosen to keep this fix
   small and dependency-free.

## Files changed

- `src/Core/API/VirtualDesktopAPI-Win10.cs`
- `src/Core/API/VirtualDesktopAPI-Win10LTSC.cs`
- `src/Core/API/VirtualDesktopAPI-Win11.cs`
- `src/Core/API/VirtualDesktopAPI-Win11_.cs`
- `src/Core/Types.cs`
- `src/Core/WindowManager.cs`
- `src/GUI/VirtualDesktop/Desktop.cs`
- `src/GUI/MainForm.cs` (pinning comment/behavior clarification)
- `src/Publish.cmd` (framework-dependent release packaging)
- `README.md` (requirements and installation instructions)
- `THIRD-PARTY-NOTICES.md` (removed notices for unbundled .NET runtime assemblies)
