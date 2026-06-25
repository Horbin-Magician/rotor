const KEY_REPLACEMENTS: Array<[RegExp, string]> = [
  [/\bKey/g, ''],
  [/\bDigit/g, ''],
  [/\bEscape\b/gi, 'Esc'],
  [/\bSuper\b/gi, 'Cmd'],
  [/\bcontrol\b/gi, 'Ctrl'],
  [/\bctrl\b/gi, 'Ctrl'],
  [/\balt\b/gi, 'Alt'],
  [/\bshift\b/gi, 'Shift'],
]

const MODIFIER_ORDER = ['Cmd', 'Ctrl', 'Alt', 'Shift']

export function formatShortcut(shortcut: string) {
  const value = KEY_REPLACEMENTS.reduce((currentValue, [pattern, replacement]) => {
    return currentValue.replace(pattern, replacement)
  }, shortcut)

  const modifiers: string[] = []
  const nonModifiers: string[] = []

  value.split('+').forEach((part) => {
    const normalizedPart = part.trim()
    if (!normalizedPart) {
      return
    }

    const modifier = MODIFIER_ORDER.find(
      (item) => item.toLowerCase() === normalizedPart.toLowerCase(),
    )
    if (modifier) {
      modifiers.push(modifier)
    } else {
      nonModifiers.push(normalizedPart)
    }
  })

  modifiers.sort((a, b) => MODIFIER_ORDER.indexOf(a) - MODIFIER_ORDER.indexOf(b))
  return [...modifiers, ...nonModifiers].join('+')
}
