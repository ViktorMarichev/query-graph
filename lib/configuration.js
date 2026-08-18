'use strict'

function objectOf(label, value) {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) {
    throw new TypeError(`${label} must be an object`)
  }
  return value
}

function configurationOf(factory, value, allowedKeys) {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) {
    throw new TypeError(`${factory} expects a configuration object`)
  }

  const unknownKeys = Reflect.ownKeys(value).filter((key) => typeof key !== 'string' || !allowedKeys.has(key))
  if (unknownKeys.length > 0) {
    const label = unknownKeys.length === 1 ? 'field' : 'fields'
    const keys = unknownKeys.map((key) => JSON.stringify(String(key))).join(', ')
    throw new TypeError(`${factory} received unknown configuration ${label} ${keys}`)
  }

  return value
}

function nonEmptyString(value, label) {
  if (typeof value !== 'string' || value.trim() === '') {
    throw new TypeError(`${label} must be a non-empty string`)
  }
  return value
}

function optionalBoolean(value, label) {
  if (value !== undefined && typeof value !== 'boolean') {
    throw new TypeError(`${label} must be a boolean`)
  }
  return value
}

function optionalEnum(value, allowedValues, label) {
  if (value !== undefined && !allowedValues.has(value)) {
    const choices = [...allowedValues].map((choice) => JSON.stringify(choice)).join(' or ')
    throw new TypeError(`${label} must be ${choices}`)
  }
  return value
}

function arrayOf(value, label) {
  if (!Array.isArray(value)) {
    throw new TypeError(`${label} must be an array`)
  }
  return value
}

function optionalArray(value, label) {
  return value === undefined ? undefined : arrayOf(value, label)
}

module.exports = {
  arrayOf,
  configurationOf,
  nonEmptyString,
  objectOf,
  optionalArray,
  optionalBoolean,
  optionalEnum,
}
