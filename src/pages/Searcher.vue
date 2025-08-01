<template>
    Hello
</template>

<script setup lang="ts">

import { getCurrentWindow } from '@tauri-apps/api/window'
const appWindow = getCurrentWindow()
appWindow.isVisible().then( (visible)=>{
  if(visible == false) {
    appWindow.show()
    appWindow.setFocus()
  }
})

</script>


<style scoped>

</style>
