import {_getAppVersionCache} from "~/common/store.ts";

let loaded: boolean = false
let version = "0.0.0"

export function loadModule(autoTrack: boolean = true): Promise<void> {
    return new Promise((resolve, reject) => {

        const script = document.createElement('script')
        script.setAttribute('src', 'https://cloud.umami.is/script.js')
        script.setAttribute('data-website-id', 'f42bb87d-98e5-4ee5-aa33-1ac21dbb42a2')
        script.setAttribute('data-cache', 'true')
        script.setAttribute('data-auto-track', autoTrack ? 'true' : 'false')
        script.onload = () => {
            loaded = true
            resolve()
        }
        script.onerror = () => {
            loaded = false
            reject()
        }
        document.body.appendChild(script)
    })
}

export function eventGlobalData(): Record<string, any> {
    return {
        version: _getAppVersionCache()
    }
}

export async function trackEvent(event: string, eventData?: Record<string, any>) {
    try{
        if (loaded && window.umami) {
            let data = {
                ...eventGlobalData(),
                ...eventData
            }
            await window.umami.track(({website, language}) => ({
                language,
                website,
                name: event,
                data
            }))
        }
    } catch (e) {
        console.error(e)
    }
}