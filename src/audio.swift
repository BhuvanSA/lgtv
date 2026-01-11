import CoreAudio

func getDefaultAudioDevice() -> AudioDeviceID {
    var deviceId: AudioDeviceID = kAudioObjectUnknown
    var size = UInt32(MemoryLayout.size(ofValue: deviceId))
    var address = AudioObjectPropertyAddress(
        mSelector: kAudioHardwarePropertyDefaultOutputDevice,
        mScope: kAudioObjectPropertyScopeGlobal,
        mElement: kAudioObjectPropertyElementMain
    )

    let error = AudioObjectGetPropertyData(
        AudioObjectID(kAudioObjectSystemObject),
        &address,
        0,
        nil,
        &size,
        &deviceId
    )

    if error != noErr {
        return kAudioObjectUnknown
    }
    return deviceId
}

func getDeviceName(deviceId: AudioDeviceID) -> String? {
    var nameSize: UInt32 = 0
    var address = AudioObjectPropertyAddress(
        mSelector: kAudioDevicePropertyDeviceNameCFString,
        mScope: kAudioObjectPropertyScopeGlobal,
        mElement: kAudioObjectPropertyElementMain
    )
    
    // Get size first
    var error = AudioObjectGetPropertyDataSize(
        deviceId,
        &address,
        0,
        nil,
        &nameSize
    )
    
    if error != noErr { return nil }

    var name: CFString = "" as CFString
    error = AudioObjectGetPropertyData(
        deviceId,
        &address,
        0,
        nil,
        &nameSize,
        &name
    )

    if error != noErr { return nil }
    return name as String
}

let deviceId = getDefaultAudioDevice()
if deviceId != kAudioObjectUnknown {
    if let name = getDeviceName(deviceId: deviceId) {
        print(name)
        exit(0)
    }
}
exit(1)
