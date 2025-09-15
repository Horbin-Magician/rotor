<p align="center"><a href="https://github.com/Horbin-Magician/rotor-rs" target="_blank" rel="noopener noreferrer"><img width="100" src="./public/assets/logo.png" alt="Rotor logo"></a></p>

<p align="center">
<span>English</span>
<span> | </span>
<a href="doc\README_CN.md">中文</a>
</p>

<p align="center"><span>A fast, low occupancy and free toolbox for Windows and MacOS.</span></p>

<div align="center">

![GitHub License](https://img.shields.io/github/license/Horbin-Magician/rotor?style=flat)
![GitHub Downloads (all assets, all releases)](https://img.shields.io/github/downloads/Horbin-Magician/rotor/total?style=flat)
![Windows Support](https://img.shields.io/badge/Windows-0078D6?style=flat&logo=windows&logoColor=white)
![macOS Support](https://img.shields.io/badge/macOS-000000?style=flat&logo=apple&logoColor=white)

</div>

# What is Rotor?

Rotor is a **fast**, **low occupancy** and **free** toolbox running in windows and MacOS.

Now, Rotor include **file search** and **screenshot** module.

# Modules

## File Searching

1. Shortcut key `CMD+Shift+F` shows the search window.
2. Next, enter any information in the search window to get the search results immediately.
3. `Up` and `Down` keys to select the result, `Enter` key to open the file. 
4. A menu is displayed when the mouse hovers over the result item. You can opening the directory where the file is located or run as admin.

<div align=center>
<img src="./doc/search_demo.png" width="500" height="470"> 
</div>

## Screenshot

1. Shortcut key `CMD+Shift+S` prints screen.
2. Then by holding down the `left mouse button` drag to select the area you want to take a screenshot, release to complete;
3. By default, the captured image is fixed to the screen, and press `ESC` to close it, `S` to save it, `Enter` to copy it, `H` to minimize it.

<div align=center>
<img src="./doc/screenshot_demo.png" width="558" height="352"> 
</div>

# Contribution

This project is builded by [Rust](https://www.rust-lang.org/), and use [Tauri](https://github.com/tauri-apps/tauri/) as GUI framwork.

I sincerely hope that you can provide quality code for this project.

## Important to-do list

- [x] Optimize the user experience under the Windows OS;
- [x] A brand new update system;
- [x] OCR function for screenshots, directly scratch and copy the text in the image just like WeChat;
- [ ] Word lookup and translation function;
- [ ] Basic query-based AI dialogue function;
- [ ] Plug-in system.

# License

[MIT](https://opensource.org/licenses/MIT)

Copyright (c) 2022-present, Horbin

