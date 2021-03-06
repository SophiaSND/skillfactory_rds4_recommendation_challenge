let filter = { }

let filter_original = { }

function prepareFilterSections() {
    const header_elem = document.querySelector("#filter > header")
    const footer_elem = document.querySelector("#filter > footer")
    const main_elem = document.querySelector("#filter > main")
    const list_ul = document.querySelectorAll("#filter > main > section > ul[data-data]")
    list_ul.forEach(ul => {
        const list_item = JSON.parse(ul.dataset.data)
        const height_stop = !ul.dataset.heightMax ? null : (
            document.documentElement.clientHeight -
            (header_elem.clientHeight + footer_elem.clientHeight + (ul.getBoundingClientRect().top - main_elem.getBoundingClientRect().top))
        ) * parseFloat(ul.dataset.heightMax)
        const name = ul.dataset.name

        if (ul.dataset.selected) {
            let json = JSON.parse(ul.dataset.selected)
            if (Array.isArray(json)) {
                filter[name] = new Set(json)
                filter_original[name] = new Set(json)
            } else {
                filter[name] = json
                filter_original[name] = json
            }
        }
        let is_truncated = false
        for (let item of list_item) {
            const li = appendLiFromItem(ul, item, name)
            if (height_stop && ul.clientHeight > height_stop) {
                ul.removeChild(ul.lastChild)
                is_truncated = true
                break
            }
        }
        if (is_truncated) {
            const header = ul.previousElementSibling
            const el = document.createElement('label')
            el.dataset.state = 'less'
            el.innerText = 'show more'
            el.onclick = showMoreClick
            header.appendChild(el)
        }
    })
}

function appendLiFromItem(ul, item, name) {
    const li = document.createElement("li")
    li.dataset.id = item.id
    li.innerText = item.name
    if (filter.hasOwnProperty(name)) {
        if (filter[name] instanceof Set) {
            if (filter[name].has(item.id + '')) {
                li.dataset.selected = 'true'
            }
        } else {
            if (filter[name] == item.id) {
                li.dataset.selected = 'true'
            }
        }
    }
    ul.appendChild(li)
    return li
}

function showMoreClick(event) {
    const label = event.target
    const ul = label.parentElement.nextElementSibling
    if (label.dataset.state == 'less') {
        const name = ul.dataset.name
        const list_item = JSON.parse(ul.dataset.data)
        const skip_count = ul.children.length
        let i = 0
        for (let item of list_item) {
            if (i < skip_count) {
                i++
            } else {
                appendLiFromItem(ul, item, name)
            }
        }
        label.dataset.lessCount = skip_count
        label.dataset.state = 'more'
        label.innerText = 'show less'
        const section = ul.parentElement
        const main = section.parentElement
        const footer = main.nextElementSibling
        const height_max = footer.offsetTop - main.offsetTop - 48
        if (ul.offsetHeight > height_max) {
            ul.style.cssText = 'height: ' + height_max + 'px; overflow-y: scroll'
        }
        main.scrollTo({left: 0, top: section.offsetTop - main.offsetTop - 4, behavior: 'smooth'})
    } else {
        let less_count = label.dataset.lessCount
        while (ul.children.length > less_count) {
            ul.removeChild(ul.lastChild)
        }
        label.dataset.state = 'less'
        label.innerText = 'show more'
        if (ul.style.cssText != '') {
            ul.style.cssText = ''
        }
    }
}

function ulClick(event) {
    if (event.target.tagName == "LI") {
        const li = event.target
        const ul = li.parentElement
        const name = ul.dataset.name
        const id_selected = li.dataset.id
        if (ul.dataset.type == 'multi') {
            const id_current = filter[name]
            if (id_current && id_current.has(id_selected)) {
                li.dataset.selected = "false"
                filter[name].delete(id_selected)
            } else {
                li.dataset.selected = "true"
                if (id_current) {
                    filter[name].add(id_selected)
                } else {
                    filter[name] = new Set([id_selected])
                }
            }
        } else {
            const id_current = filter[name]
            if (id_selected != id_current) {
                filter[name] = id_selected
                Array.prototype.forEach.call(ul.children, li => {
                    if (li.dataset.id == id_selected) {
                        li.dataset.selected = "true"
                    } else {
                        li.dataset.selected = "false"
                    }
                })
            } else {
                li.dataset.selected = "false"
                delete filter[name]
            }
        }
    }
}

function applyFilter(urlFilterMain, urlSearch) {
    console.log('url')
    const header = document.querySelector('body > header')
    const footer = document.querySelector('body > footer')
    const h1 = document.querySelector('h1')
    const filter_elem = document.getElementById('filter')
    const main = document.querySelector('body > main')
    if (filter_elem.dataset.state == "closed") {
        const rect = filter_elem.getBoundingClientRect()
        let cssText = 'top: -' + Math.round(rect.top) + 'px'
        header.style.cssText = cssText
        main.style.cssText = cssText
        footer.style.cssText = cssText
        h1.style.cssText = cssText
        filter_elem.style.cssText = cssText + '; max-height: 100vh; height: 100vh'
        document.querySelector('#filter > footer').style.cssText = 'display: flex'
        setTimeout(function(){
            cssText = cssText + '; display: none'
            header.style.cssText = cssText
            main.style.cssText = cssText
            footer.style.cssText = cssText
            h1.style.cssText = cssText
            filter_elem.style.cssText = 'position: static'
            filter_elem.dataset.state = "opened"
        }, 500)
        url = urlFilterMain + window.location.search
        fetch(url)
            .then((response) => response.text())
            .then((text) => {
                const main = document.querySelector('#filter > main')
                main.innerHTML = text
                prepareFilterSections()
            })
    } else {
        const cssTop = header.style.cssText.split(';')[0]
        const cssText = cssTop + '; display: block'
        header.style.cssText = cssText
        main.style.cssText = cssText
        footer.style.cssText = cssText
        h1.style.cssText = cssText
        filter_elem.style.cssText = cssTop + '; transition: top 0s; position: relative'
        document.querySelector('#filter > footer').style.cssText = ''
        setTimeout(function() {
            header.style.cssText = ''
            main.style.cssText = ''
            footer.style.cssText = ''
            h1.style.cssText = ''
            filter_elem.style.cssText = ''
            filter_elem.dataset.state = "closed"
            const props_filter = new Set(Object.getOwnPropertyNames(filter))
            const props_filter_original = new Set(Object.getOwnPropertyNames(filter_original))
            let is_changed = is_diff_sets(props_filter, props_filter_original)
            if (!is_changed) {
                for (const prop of props_filter) {
                    if (is_diff_sets(filter[prop], filter_original[prop])) {
                        is_changed = true
                        break
                    }
                }
            }
            if (is_changed) {
                const main_elem = document.querySelector('body > main')
                main_elem.innerHTML = ''
                setTimeout(function() {
                    const params = []
                    for (const name in filter) {
                        const value = filter[name]
                        if (value instanceof Set) {
                            const values = Array.from(value)
                            if (values.length > 0) {
                                params.push(name + '=' + values.join(','))
                            }
                        } else {
                            params.push(name + '=' + value)
                        }
                        console.log(name, value)
                    }
                    window.location.href = urlSearch + params.join('&')
                }, 500)
            }
        }, 1)
    }
}

function clearFilter() {
    console.log('clearFilter')
}

function is_diff_sets(setA, setB) {
    if (!(setA instanceof Set)) setA = new Set([setA])
    if (!(setB instanceof Set)) setB = new Set([setB])
    for (const item of setA) {
        if (!setB.has(item)) {
            return true
        }
    }
    for (const item of setB) {
        if (!setA.has(item)) {
            return true
        }
    }
    return false
}
