#pragma once
#include <Arduino.h>

// One layout definition shared by receiver and bootloader. Never infer flash
// density from STM32F407xx/STM32F103xE: those macros cover several densities.
#ifndef FLPROG_STM32_OTA_LAYOUT
#if defined(STM32F407xx)
#define FLPROG_STM32_OTA_LAYOUT 1
#else
#define FLPROG_STM32_OTA_LAYOUT 0
#endif
#endif

#if FLPROG_STM32_OTA_LAYOUT == 1 || FLPROG_STM32_OTA_LAYOUT == 2
#if !defined(STM32F407xx)
#error "F407 OTA layout requires STM32F407xx"
#endif
#include <stm32f4xx_hal.h>
#elif FLPROG_STM32_OTA_LAYOUT == 3 || FLPROG_STM32_OTA_LAYOUT == 4
#if !defined(STM32F103xE)
#error "F103 256/512 KiB OTA requires STM32F103xE"
#endif
#include <stm32f1xx_hal.h>
#else
#error "No implemented STM32 OTA flash layout selected"
#endif

namespace FlprogOtaFlash {
constexpr uint32_t base = 0x08000000UL;
#if FLPROG_STM32_OTA_LAYOUT == 1
constexpr uint32_t flashKiB=1024, app=base+0x10000, stage=base+0x80000;
constexpr uint32_t ramEnd=0x20020000UL, appSector=4, appSectors=4, stageSector=8, stageSectors=4;
#elif FLPROG_STM32_OTA_LAYOUT == 2
constexpr uint32_t flashKiB=512, app=base+0x10000, stage=base+0x40000;
constexpr uint32_t ramEnd=0x20020000UL, appSector=4, appSectors=2, stageSector=6, stageSectors=2;
#elif FLPROG_STM32_OTA_LAYOUT == 3
constexpr uint32_t flashKiB=256, app=base+0x8000, stage=base+0x20000;
constexpr uint32_t ramEnd=0x2000C000UL;
#else
constexpr uint32_t flashKiB=512, app=base+0x8000, stage=base+0x40000;
constexpr uint32_t ramEnd=0x20010000UL;
#endif
constexpr uint32_t end=base+flashKiB*1024UL, capacity=stage-app, data=stage+0x100;
static_assert(app > base && app < stage && data+capacity <= end, "OTA regions overlap");
inline bool hardwareMatches() {
    return *reinterpret_cast<volatile const uint16_t *>(FLASHSIZE_BASE)==flashKiB;
}
inline bool vectorValid(uint32_t location, uint32_t size) {
    if (size<8 || size>capacity) return false;
    const uint32_t sp=*reinterpret_cast<const uint32_t *>(location);
    const uint32_t pc=*reinterpret_cast<const uint32_t *>(location+4);
    return !(sp&7U) && sp>0x20000000UL && sp<=ramEnd && (pc&1U) &&
        (pc&~1UL)>=app && (pc&~1UL)<app+size;
}
// Caller owns HAL_FLASH_Unlock/Lock. Bounds exclude the bootloader.
inline bool programWord(uint32_t address,uint32_t value) {
    if ((address&3U) || address<app || address>end-4) return false;
#if FLPROG_STM32_OTA_LAYOUT <= 2
    return HAL_FLASH_Program(FLASH_TYPEPROGRAM_WORD,address,value)==HAL_OK;
#else
    return HAL_FLASH_Program(FLASH_TYPEPROGRAM_HALFWORD,address,value&0xFFFFU)==HAL_OK &&
        HAL_FLASH_Program(FLASH_TYPEPROGRAM_HALFWORD,address+2,value>>16)==HAL_OK;
#endif
}
inline bool erase(bool staging) {
    if (!hardwareMatches()) return false;
    FLASH_EraseInitTypeDef spec={}; uint32_t error=0xFFFFFFFFUL;
#if FLPROG_STM32_OTA_LAYOUT <= 2
    spec.TypeErase=FLASH_TYPEERASE_SECTORS;
    spec.VoltageRange=FLASH_VOLTAGE_RANGE_3;
    spec.Sector=staging?stageSector:appSector;
    spec.NbSectors=staging?stageSectors:appSectors;
#else
    spec.TypeErase=FLASH_TYPEERASE_PAGES;
    spec.PageAddress=staging?stage:app;
    spec.NbPages=((staging?end:stage)-spec.PageAddress)/2048UL;
#endif
    return HAL_FLASHEx_Erase(&spec,&error)==HAL_OK;
}
}
