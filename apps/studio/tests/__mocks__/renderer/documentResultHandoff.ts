/** Native transfer boundary; file writes remain on the real Documents HTTP adapter. */
export function documentResultHandoffFixture() {
  let request: { id: string; file: unknown } | null = null
  let sequence = 0
  let refuseAcknowledgement = false
  let status: { state: string; location?: string } = { state: 'pending' }
  return {
    expireCurrentRequest() {
      request = null
    },
    refuseNextAcknowledgement() {
      refuseAcknowledgement = true
    },
    async invoke(command: string, args?: Record<string, unknown>): Promise<unknown> {
      if (command === 'stage_document_result_save') {
        if (request && status.state === 'pending') throw new Error('A result is already being saved')
        request = { id: `result-request-${++sequence}`, file: args?.file }
        status = { state: 'pending' }
        return request.id
      }
      if (command === 'take_document_result_save') {
        if (!request || status.state !== 'pending') return null
        return request
      }
      if (command === 'finish_document_result_save') {
        if (!request || args?.id !== request.id) throw 'Document result request refused.'
        if (refuseAcknowledgement) {
          refuseAcknowledgement = false
          throw new Error('Native acknowledgement temporarily unavailable')
        }
        status =
          typeof args?.location === 'string' ? { state: 'saved', location: args.location } : { state: 'cancelled' }
        return null
      }
      if (command === 'document_result_save_status' && args?.id === request?.id) return status
      throw new Error('Unsupported native handoff request')
    }
  }
}
