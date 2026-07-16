const RGBA_BYTES_PER_PIXEL = 4

const ONE_PIXEL_DIMENSION_OFFSETS = [
  [-1, 0],
  [1, 0],
  [0, -1],
  [0, 1],
  [-1, -1],
  [-1, 1],
  [1, -1],
  [1, 1],
] as const

export interface RgbaImageDataResult {
  imageData: ImageData
  width: number
  height: number
  corrected: boolean
}

function assertValidDimension(value: number, name: string) {
  if (!Number.isSafeInteger(value) || value <= 0) {
    throw new Error(`Invalid RGBA image ${name}: ${value}`)
  }
}

export function createValidatedRgbaImageData(
  buffer: ArrayBuffer,
  expectedWidth: number,
  expectedHeight: number,
): RgbaImageDataResult {
  assertValidDimension(expectedWidth, 'width')
  assertValidDimension(expectedHeight, 'height')

  const expectedByteLength = expectedWidth * expectedHeight * RGBA_BYTES_PER_PIXEL
  let width = expectedWidth
  let height = expectedHeight
  let corrected = false

  if (buffer.byteLength !== expectedByteLength) {
    if (buffer.byteLength % RGBA_BYTES_PER_PIXEL !== 0) {
      throw new Error(
        `Invalid RGBA image data: received ${buffer.byteLength} bytes, which is not a whole number of RGBA pixels`,
      )
    }

    const matchingDimensions = ONE_PIXEL_DIMENSION_OFFSETS.map(([widthOffset, heightOffset]) => ({
      width: expectedWidth + widthOffset,
      height: expectedHeight + heightOffset,
    })).find(
      (candidate) =>
        candidate.width > 0 &&
        candidate.height > 0 &&
        candidate.width * candidate.height * RGBA_BYTES_PER_PIXEL === buffer.byteLength,
    )

    if (!matchingDimensions) {
      throw new Error(
        `Invalid RGBA image data: received ${buffer.byteLength} bytes; expected ${expectedByteLength} bytes for ${expectedWidth}x${expectedHeight}, and no dimensions within 1 pixel match`,
      )
    }

    width = matchingDimensions.width
    height = matchingDimensions.height
    corrected = true
  }

  return {
    imageData: new ImageData(new Uint8ClampedArray(buffer), width, height),
    width,
    height,
    corrected,
  }
}
