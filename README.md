# Micro Task Pager

<p align="center">
  <img src="src/Resources/logo.svg" width="600px">
</p>

## Description
*Micro Task Pager* is a desktop pager for Windows, inpsired by various Linux desktop environments and their plenty of available tools. Those pager apps make the handling of multiple (virtual) desktops easier. It allows you to switch to any desktop on a single click, as well as drag & drop app windows between the desktops. Traditionally those apps visualize all your desktop windows in a small thumbnail representation, so you can easily see which layout is used on which desktop, including the windows z-order. _Micro Task Pager_ supports all those features but adds also an additional alternative _Micro Taskbar_ view of a desktop like - as the name suggests - works like a mini taskbar. So you can easily see which programs are opened where, independent from their screen location or if they are maximized.

Those _Micro Taskbar_ views are especially useful on small screens like notebooks, there isn't much space for more than 2 or 3 windows in a layout, most of them are rather used maximized and are stacked above each other. Using this approach, you don't loose the big picture, your apps are also more easily recognized by their icons.

<img src="src/Resources/pager-example.png">

## Background
Microsoft has introduced native virtual desktop support in Windows 10 so special tools were no longer required to make use of workflows based on multiple desktops. Especially when working on an intensive task with a lot of windows open and you need to switch context unrelated to the current task but you don't want to minimize all windows and lose your train of thought...

Unfortunately the implementation in Windows isn't very flexible and all keyboard shortcuts are tightly integrated in the system. Also... 

- UI/UX for switching between isn't so efficient in Windows (even though there are improvements in Win 11) :
	- there are only a few fixed hotkeys to swap between desktops
	- no realtime view of all your windows
	- to get to the official UI you have to press Win+Tab, a sometimes weird hotkey that gets frequently confused with others

> [!NOTE]
> Unlike some classic Linux tools (e.g. _FvwmPager_) the app doesn't support _pages_ because Windows has no native support for handling oversized desktops

## Features
- Jump to any native (virtual) desktop using a simple mouse click
- place the pager anywhere you want: stick to screen sides/corners, put above or even over the Windows taskbar
- near realtime view of the windows in all virtual desktops using a small thumbnail/preview of their location, z-order and icon
- decide want you want to see for each desktop on the pager: a thumbnail preview of the openend windows or a _Micro Taskbar_
- ability to *drag and drop* windows between virtual desktops
- each desktop remembers the last active window which is re-activated when use comes back to it 
- supports multiple screens connected to the PC
- supports High-DPI displays
- change any parameter to your like: Pager height, icon padding, selection style, colors, responsivness
- tested with the latest Windows 11 versions including 26H2
- build upon the native features of Windows 10/11 to enhance the experience, no system hacks needed
- because of the last point it should work well together with other desktop tools / window managers like FanyZones
- all free and open source

## Known issues
- Not every parameter is user-configurable yet
- Sometimes newly opened windows are not detected by the application
- The app is still based on a pretty old _.NET_ version from 2019 so there is no optimized build for ARM64 yet (but it works there using the x86 emulation layer)
- the order of the icons in the _Micro Taskbar_ view are currently not changeable
- the app isn't signed yet so you will probably see a security warning from Windows when starting it

## Acknowledgements
- [Switchie](https://github.com/darkguy2008/Switchie) (the abandoned original project that *Micro Task Pager* is based on)
- [VirtualDesktop](https://github.com/MScholtes/VirtualDesktop) (thanks to this project we have desktop management also in recent versions of Windows 10/11)

Made with ♥ from Munich, Germany
