// Minimal WebView2 host object typings for SharedBuffer transfers.

interface WebView2SharedBufferReceivedEvent {
  getBuffer(): ArrayBuffer
  // Parsed from the JSON string passed to PostSharedBufferToScript;
  // undefined when the native side posted no additional data.
  additionalData?: unknown
}

interface WebView2HostObject {
  addEventListener(
    type: 'sharedbufferreceived',
    listener: (event: WebView2SharedBufferReceivedEvent) => void,
  ): void
  removeEventListener(
    type: 'sharedbufferreceived',
    listener: (event: WebView2SharedBufferReceivedEvent) => void,
  ): void
  releaseBuffer(buffer: ArrayBuffer): void
}

interface Window {
  chrome?: {
    webview?: WebView2HostObject
  }
}
