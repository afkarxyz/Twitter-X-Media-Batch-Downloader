[![GitHub All Releases](https://img.shields.io/github/downloads/afkarxyz/Twitter-X-Media-Batch-Downloader/total?style=for-the-badge)](https://github.com/afkarxyz/Twitter-X-Media-Batch-Downloader/releases)

![TwitterXMediaBatchDownloader](https://github.com/user-attachments/assets/354d7470-c01c-4aa6-9da1-ea6c42d27330)

<div align="center">
<b>Twitter/X Media Batch Downloader</b> is a GUI tool that allows users to download all media, including images and videos, in their original quality from Twitter/X accounts using <code>gallery-dl</code>
</div>

## Download

- Download the latest version of [TwitterXMediaBatchDownloader](https://github.com/afkarxyz/Twitter-X-Media-Batch-Downloader/releases/download/v1.2/TwitterXMediaBatchDownloader.exe)
- If you're familiar with **userscripts**, please install it [here](https://greasyfork.org/en/scripts/523157-twitter-x-media-batch-downloader)
- If you want to download media from a **suspended account**, use the userscript version.

## Features

- Uses the powerful `gallery-dl` library, similar to `yt-dlp`  
- Simple and compact GUI
- Download images and videos in original quality
- Option to choose the media type to download: **media** (image + video) or just **image/video**
- **Batch** download settings, ranging from 10 to 100 files per download  
- Filename format customization
  
## Screenshots

![image](https://github.com/user-attachments/assets/ccdd8a8d-890f-4d3c-a3b1-56dad3eb82a9)

![image](https://github.com/user-attachments/assets/01a80593-59fe-4c2b-a325-7e401fa3048f)

![image](https://github.com/user-attachments/assets/1779659e-3512-4e74-be0a-088419267fe0)

## How to Obtain Auth Token

> [!Warning]
> I suggest not using the **main account** to obtain the token.

1. Go to [Twitter's website](https://www.x.com/)
2. Log into your account
3. Open the Developer Tools by pressing `F12`
4. Navigate to the **Application** tab, then select **Storage** > **Cookies**
5. Find and copy the `auth_token` value or use the browser extension [Cookie-Editor](https://cookie-editor.com/)
6. Do not log out of your account, as a new `auth_token` will be generated by Twitter
   
![image](https://github.com/user-attachments/assets/50f819da-7490-4f3c-b130-c5a3ee482e2d)
