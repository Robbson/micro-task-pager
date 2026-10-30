# Rust Migration – Architecture and Development Guidelines

## 1. Project Overview

This project is a migration of an existing C#/.NET Windows Forms application to native Rust.

The application is a **Virtual Desktop Pager for Windows**. Its primary purpose is managing Windows virtual desktops and providing a visual overview of them, including window previews and application icons.

The application is heavily Windows-specific and does not need to be cross-platform.

The main goals of the migration are:

- Eliminate the .NET runtime dependency.
- Use Rust as the primary implementation language.
- Improve memory safety and reliability.
- Retain access to native Windows functionality.
- Preserve existing application behavior and functionality.
- Modernize selected UI components where it makes sense.
- Keep the architecture modular and maintainable.

Microsoft's official `windows-rs` ecosystem should be preferred wherever practical.

## 2. Existing Application Characteristics

The existing application has two main UI areas:

### Main Form

The MainForm.cs and the Desktop.cs as well as the ScreenRenderer.cs are largely custom-rendered using GDI.
They display virtual desktops, window previews, application icons and other visual elements.

Painting is relatively simple and primarily consists of:

- Filled rectangles.
- Bitmaps.
- Application icons.
- Window previews.
- Custom layout and visual state representation.

The application does not rely heavily on standard WinForms controls.

### Settings Panel

The settings panel is comparatively small and independent of the main rendering functionality.
It is a good candidate for modernization using native Windows UI technologies.
The settings model and application configuration must remain independent of the UI implementation.

### Windows Virtual Desktop Management

The most technically challenging component is the management of Windows virtual desktops.
The application relies on undocumented Windows Shell COM interfaces, particularly internal Virtual Desktop Manager interfaces.
These interfaces may change between Windows releases and feature updates.
Supporting multiple Windows versions is therefore an important architectural consideration.

---

## 3. Technology Stack

After a first look, the currently preferred technology stack seems to be:

| Area | Technology |
|---|---|
| Programming language | Rust |
| Windows API bindings | `windows` / `windows-rs` |
| Low-level Windows bindings | `windows-sys` where necessary |
| COM interoperability | `windows-core` |
| Main window | Native Win32 |
| Main rendering | Direct2D via `windows-canvas` |
| Settings UI | `winsafe` with classic Win32 controls because `windows-reactor` for WinUI 3 isn't stable yet |
| Window previews | DWM Thumbnail API |
| Application icons | Windows Shell APIs |
| Build system | Cargo |

Use official Microsoft Rust libraries whenever possible.
Avoid introducing large third-party GUI frameworks unless there is a compelling technical reason.

## 4. Repository Structure

The Rust implementation should live in a separate directory inside the existing Git repository.
The existing C# implementation can remain available during the migration.
Start with a single Rust crate. Split it into multiple crates only when there is a concrete architectural benefit.

---

## 5. Windows API Integration

### 5.1 windows-rs

Use Microsoft's official `windows` crate as the primary Windows API binding.

It provides access to classic Win32 functionality, including:
- Window creation and management.
- Window procedures.
- Message loops.
- Win32 controls.
- GDI.
- DWM.
- Shell APIs.
- COM.
- Windows Runtime APIs.

Use `windows-sys` only when lower-level bindings are necessary or preferable.
Keep unsafe code localized and encapsulated behind safe Rust interfaces.

### 5.2 Win32 Main Window

The main application window should initially remain a classic Win32 window. Other opinions?

Implement the required functionality using:

- `RegisterClassExW`
- `CreateWindowExW`
- `DefWindowProcW`
- `GetMessageW`
- `DispatchMessageW`
- `WM_CREATE`
- `WM_SIZE`
- `WM_COMMAND`
- `WM_PAINT`
- `WM_DESTROY`

The window procedure should remain relatively small.
Separate application logic, event handling and rendering from the actual Win32 message dispatch.

---

## 6. Virtual Desktop Management

This is the most important and Windows-specific subsystem.
The application uses undocumented Windows Shell COM interfaces to manage virtual desktops.
Relevant interfaces include:

- `IVirtualDesktopManager`
- `IVirtualDesktopManagerInternal`
- `IVirtualDesktop`
- `IVirtualDesktopNotification`
- `IVirtualDesktopNotificationService`

The public `IVirtualDesktopManager` interface is documented, but several capabilities required by a full virtual desktop pager are only available through undocumented internal interfaces which can be found in die Core/API Folder of the C# implementation.

Do not assume that every required internal COM interface is available through Microsoft's generated Windows bindings.
Do not scatter version-specific COM definitions throughout the application.
Use separate implementation modules for supported Windows versions.

## 7. Rendering: Direct2D Instead of GDI

The existing GDI rendering implementation should be evaluated for migration to Direct2D.
The preferred Rust library is Microsoft's `windows-canvas` crate.
It provides a higher-level Rust graphics abstraction built around Windows graphics technologies.
Direct2D is particularly suitable for this application because the rendering requirements are relatively simple:

- Filled rectangles.
- Bitmap rendering.
- Application icons.
- Window previews.
- Scaling and clipping.
- Basic visual effects.

### 7.1 GDI to Direct2D Mapping

| Existing GDI operation | Direct2D equivalent |
|---|---|
| `FillRect` | `FillRectangle` |
| `Rectangle` | `DrawRectangle` |
| `BitBlt` | `DrawBitmap` |
| `CreateSolidBrush` | `CreateSolidColorBrush` |
| `StretchBlt` | `DrawBitmap` with destination rectangle |

### 7.2 Rendering Architecture

Keep rendering independent of application logic.
The renderer should receive the current application state and translate it into drawing operations.
Avoid mixing COM calls, desktop management and drawing code.

Use GPU resources efficiently:

- Reuse brushes.
- Cache bitmaps.
- Avoid unnecessary resource recreation.
- Handle device loss.
- Recreate device-dependent resources when necessary.

### 7.3 Window Integration

Initially, use a classic Win32 HWND for the main window.
Investigate hosting a Direct2D rendering surface through an appropriate swap chain.
Do not introduce WinUI merely to obtain hardware-accelerated rendering.
A traditional Win32 window with Direct2D rendering is a valid and potentially simpler architecture.
Use demand-driven rendering rather than a permanent render loop if the application does not require continuous animation.

The application should redraw when its visual state changes.


## 8. Settings UI: WinUI 3 with windows-reactor

The settings panel is relatively small and independent of the main rendering implementation.
Currently it's a built programmatic win form so we should stick to a similar approach when using `winsafe`

### 8.1 Separate Settings Window

Keep the settings model independent.
The actual model should be derived from the existing application's configuration requirements.


## 9. Migration Strategy

Do not rewrite the entire application at once.
Use an incremental migration.

### Phase 1: Repository and Build Setup

- Initialize a Cargo project in the `rust/` directory.
- Configure the required Windows API features.
- Establish a working Rust development and debugging environment.
- Keep the existing C# application intact.

### Phase 2: Virtual Desktop Subsystem

Prioritize the technically most challenging component.

- Investigate the existing C# COM implementation.
- Port the COM interfaces to Rust.
- Implement version-specific compatibility modules.
- Build a standalone test application.
- Verify functionality on all supported Windows builds.

Do not proceed with a complete UI migration until the core virtual desktop functionality works reliably.

### Phase 3: Window Metadata and Previews

- Port window enumeration.
- Port application icon retrieval.
- Implement DWM thumbnail management.
- Establish a stable data model for windows and previews.

### Phase 4: Main Window and Rendering

- Implement the native Win32 main window.
- Establish the message loop.
- Integrate `windows-canvas`.
- Port the existing GDI drawing operations to Direct2D.
- Verify DPI behavior and rendering performance.

### Phase 5: Settings UI

- Implement the independent settings model.
- Create the settings window.
- Preserve existing configuration compatibility.

### Phase 6: Final Migration

- Verify functional parity with the original C# implementation.
- Test resource management and application shutdown.
- Verify Windows version compatibility.
- Test deployment without a .NET runtime dependency.
- Remove the old C# implementation when the Rust implementation is sufficiently mature.

---

## 10. General Rust Development Guidelines

The following principles should be followed throughout development:

1. Prefer safe Rust APIs over direct FFI.
2. Keep `unsafe` blocks small and document their safety requirements.
3. Encapsulate all undocumented COM interfaces.
4. Avoid leaking raw Windows handles across application layers.
5. Use RAII wrappers for GDI, COM and other Windows resources.
6. Handle HRESULT errors explicitly.
7. Respect COM apartment and thread-affinity requirements.
8. Avoid unnecessary dependencies.
9. Keep application logic independent of UI frameworks.
10. Write compatibility tests for undocumented Windows APIs.
11. Do not assume Windows implementation details remain stable across updates.
12. Prefer native Windows functionality over unnecessary abstractions.

## 11. Important Architectural Decisions

The following decisions are the initial architectural direction, not irreversible requirements:

| Decision | Initial direction |
|---|---|
| Repository layout | Separate `rust/` directory |
| Rust project | Single Cargo crate initially |
| Main window | Classic Win32 |
| Main rendering | Direct2D via `windows-canvas` |
| Settings UI | Win32 controls via `winsafe` |
| Virtual desktops | Custom version-specific COM bindings |
| Window previews | DWM Thumbnail API |
| Application icons | Windows Shell APIs |
| Application state | Pure Rust |
| .NET dependency | Remove completely in final implementation |

## 12. Instructions for AI Coding Assistants

When assisting with this project:

- Read this architecture document before proposing structural changes.
- Prefer Microsoft's official Rust crates.
- Verify current crate APIs against their actual documentation and source code.
- Do not invent APIs, methods, COM interfaces or VTable layouts.
- Pay particular attention to undocumented Windows interfaces.
- Never assume that a COM interface definition is identical across Windows builds.
- Explain all required `unsafe` operations and their safety assumptions.
- Keep version-specific Windows compatibility code isolated.
- Avoid unnecessary architectural complexity.
- Prefer small, independently testable implementation steps.
- Preserve existing application behavior unless explicitly instructed otherwise.

When porting existing C# functionality, use the existing implementation as the behavioral reference.

The primary objective is a reliable, maintainable, fully native Windows application implemented in Rust, not merely a line-by-line translation of the original C# code.
