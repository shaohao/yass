new Vue({
    el: '#app',
    data: {
        offset: '',
        fileType: 'srt',
        subtitleFile: null,
        result: ''
    },
    methods: {
        handleFileUpload(event) {
            this.subtitleFile = event.target.files[0];
        },
        async syncSubtitle() {
            if (!this.subtitleFile) {
                alert('请选择字幕文件');
                return;
            }

            const formData = new FormData();
            formData.append('offset', this.offset);
            formData.append('file_type', this.fileType);
            formData.append('subtitle_file', this.subtitleFile);

            try {
                const response = await fetch('/sync', {
                    method: 'POST',
                    body: formData
                });

                if (!response.ok) {
                    throw new Error('同步失败');
                }

                const result = await response.json();
                this.result = result.content;
            } catch (error) {
                alert('错误：' + error.message);
            }
        },
        downloadResult() {
            const blob = new Blob([this.result], { type: 'text/plain' });
            const url = URL.createObjectURL(blob);
            const a = document.createElement('a');
            a.href = url;
            a.download = `synced_subtitle_${this.offset}.${this.fileType}`;
            document.body.appendChild(a);
            a.click();
            document.body.removeChild(a);
            URL.revokeObjectURL(url);
        }
    }
});