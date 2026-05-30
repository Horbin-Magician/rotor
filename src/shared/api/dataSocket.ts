import { getWsPort } from './core'

interface DataSocketOptions {
  onMessage?: (event: MessageEvent) => void
  retries?: number
  retryDelayMs?: number
}

interface DataSocketErrorResponse {
  requestId: number
  ok: false
  error: string
}

interface PendingDataRequest {
  label: string
  timer: number
  resolve: (data: ArrayBuffer) => void
  reject: (error: Error) => void
}

interface DataSocketState {
  nextRequestId: number
  pending: Map<number, PendingDataRequest>
}

const RESPONSE_HEADER_BYTES = 4
const MAX_REQUEST_ID = 0xFFFFFFFF
const socketStates = new WeakMap<WebSocket, DataSocketState>()

const delay = (ms: number) => new Promise(resolve => window.setTimeout(resolve, ms))

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null
}

function isDataSocketErrorResponse(value: unknown): value is DataSocketErrorResponse {
  return isRecord(value) &&
    typeof value.requestId === 'number' &&
    value.ok === false &&
    typeof value.error === 'string'
}

function allocateRequestId(state: DataSocketState) {
  let requestId = state.nextRequestId

  while (state.pending.has(requestId)) {
    requestId = requestId >= MAX_REQUEST_ID ? 1 : requestId + 1
  }

  state.nextRequestId = requestId >= MAX_REQUEST_ID ? 1 : requestId + 1
  return requestId
}

function settlePending(
  state: DataSocketState,
  requestId: number,
  settle: (request: PendingDataRequest) => void
) {
  const request = state.pending.get(requestId)
  if (!request) return

  state.pending.delete(requestId)
  window.clearTimeout(request.timer)
  settle(request)
}

function rejectPendingRequests(state: DataSocketState, error: Error) {
  state.pending.forEach((request) => {
    window.clearTimeout(request.timer)
    request.reject(error)
  })
  state.pending.clear()
}

async function getMessageBuffer(data: MessageEvent['data']) {
  if (data instanceof ArrayBuffer) return data
  if (data instanceof Blob) return data.arrayBuffer()
  return null
}

async function handleDataSocketMessage(state: DataSocketState, event: MessageEvent) {
  if (typeof event.data === 'string') {
    let response: unknown
    try {
      response = JSON.parse(event.data)
    } catch {
      rejectPendingRequests(state, new Error(`Unexpected websocket response: ${event.data}`))
      return
    }

    if (isDataSocketErrorResponse(response)) {
      settlePending(state, response.requestId, request => {
        request.reject(new Error(response.error))
      })
      return
    }

    rejectPendingRequests(state, new Error('Unexpected websocket response payload'))
    return
  }

  const buffer = await getMessageBuffer(event.data)
  if (!buffer) {
    rejectPendingRequests(state, new Error('Unexpected websocket response type'))
    return
  }

  if (buffer.byteLength < RESPONSE_HEADER_BYTES) {
    rejectPendingRequests(state, new Error('Invalid websocket data response'))
    return
  }

  const requestId = new DataView(buffer, 0, RESPONSE_HEADER_BYTES).getUint32(0)
  settlePending(state, requestId, request => {
    request.resolve(buffer.slice(RESPONSE_HEADER_BYTES))
  })
}

function getDataSocketState(socket: WebSocket) {
  let state = socketStates.get(socket)
  if (state) return state

  state = {
    nextRequestId: 1,
    pending: new Map()
  }

  socket.addEventListener('message', event => {
    void handleDataSocketMessage(state, event)
  })
  socket.addEventListener('error', () => {
    rejectPendingRequests(state, new Error('WebSocket data request failed'))
  })
  socket.addEventListener('close', () => {
    rejectPendingRequests(state, new Error('WebSocket closed while waiting for data'))
  })

  socketStates.set(socket, state)
  return state
}

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

export function requestDataSocketBytes(socket: WebSocket, label: string, timeoutMs = 1500) {
  return new Promise<ArrayBuffer>((resolve, reject) => {
    if (socket.readyState !== WebSocket.OPEN) {
      reject(new Error('WebSocket is not open'))
      return
    }

    const state = getDataSocketState(socket)
    const requestId = allocateRequestId(state)
    const timer = window.setTimeout(() => {
      state.pending.delete(requestId)
      reject(new Error(`Timed out waiting for data: ${label}`))
    }, timeoutMs)

    state.pending.set(requestId, {
      label,
      timer,
      resolve,
      reject
    })

    try {
      socket.send(JSON.stringify({ requestId, label }))
    } catch (error) {
      state.pending.delete(requestId)
      window.clearTimeout(timer)
      reject(error instanceof Error ? error : new Error(`WebSocket data request failed: ${label}`))
    }
  })
}
