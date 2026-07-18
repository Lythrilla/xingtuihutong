export {}

Component({
  properties: {
    loading: {
      type: Boolean,
      value: false,
    },
    skeleton: {
      type: Boolean,
      value: false,
    },
    error: {
      type: String,
      value: '',
    },
    empty: {
      type: Boolean,
      value: false,
    },
    emptyTitle: {
      type: String,
      value: '暂无内容',
    },
    emptyDescription: {
      type: String,
      value: '当前还没有可展示的数据',
    },
    actionText: {
      type: String,
      value: '',
    },
  },
  methods: {
    handleAction() {
      this.triggerEvent('action')
    },
  },
})
