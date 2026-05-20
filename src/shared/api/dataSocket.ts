import { getWsPort } from './core'

interface DataSocketOptions {
  onMessage?: (event: MessageEvent) => void
  retries?: number
  retryDelayMs?: number
}

const delay = (ms: number) => new Promise(resolve => window.setTimeout(resolve, ms))

function waitForOpen(socket: WebSocket) {
  return new Promise<void>((resolve, reject) => {
    const timer = window.setTimeout(() => {
      cleanup()
      reject(new Error('WebSocket open timed out'))
    }, 1000)

    const cleanup = () => {
      window.clearTimeout(timer)
      socket.removeEventListener('open', handleOpen)
      socket.removeEventListener('error', handleError)
      socket.removeEventListener('close', handleClose)
    }

    const handleOpen = () => {
      cleanup()
      resolve()
    }

    const handleError = () => {
      cleanup()
      reject(new Error('WebSocket connection failed'))
    }

    const handleClose = () => {
      cleanup()
      reject(new Error('WebSocket closed before opening'))
    }

    socket.addEventListener('open', handleOpen)
    socket.addEventListener('error', handleError)
    socket.addEventListener('close', handleClose)
  })
}

export async function connectDataSocket(options: DataSocketOptions = {}) {
  const retries = options.retries ?? 30
  const retryDelayMs = options.retryDelayMs ?? 100
  let lastError: unknown = null

  for (let attempt = 0; attempt < retries; attempt += 1) {
    const port = await getWsPort()
    const socket = new WebSocket(`ws://localhost:${port}`)
    socket.binaryType = 'arraybuffer'

    if (options.onMessage) {
      socket.addEventListener('message', options.onMessage)
    }

    try {
      await waitForOpen(socket)
      return socket
    } catch (error) {
      lastError = error
      if (options.onMessage) {
        socket.removeEventListener('message', options.onMessage)
      }
      socket.close()
      await delay(retryDelayMs)
    }
  }

  throw lastError instanceof Error ? lastError : new Error('Failed to connect data WebSocket')
}
